use crate::commands::{price, HandlerResult, MyDialogue, State};

use anyhow::{Context, Result};
use log::{error, info, warn};

use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{read, write};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

#[derive(PartialOrd, PartialEq, Clone, Serialize, Deserialize)]
pub enum Trigger {
    Lower(price::Price),
    Higher(price::Price),
    Neutral(price::Price),
}

impl Trigger {
    fn set(&mut self, price: price::Price) {
        match self {
            Self::Lower(x) | Self::Higher(x) | Self::Neutral(x) => *x = price,
        }
    }

    const fn price(&self) -> price::Price {
        match self {
            Self::Lower(x) | Self::Higher(x) | Self::Neutral(x) => *x,
        }
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lower(x) => write!(f, "менше ніж {x:.2}$"),
            Self::Higher(x) => write!(f, "більше ніж {x:.2}$"),
            Self::Neutral(_) => unreachable!(),
        }
    }
}

impl fmt::Debug for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lower(x) => write!(f, "Trigger::Lower({x:.2})"),
            Self::Higher(x) => write!(f, "Trigger::Higher({x:.2})"),
            Self::Neutral(_) => unreachable!(),
        }
    }
}

pub async fn start(bot: Bot, dialogue: MyDialogue) -> HandlerResult {
    let buttons = [
        ("Ціна більше ніж ...", "Higher"),
        ("Ціна менше ніж ...", "Lower"),
    ]
    .map(|(button, callback)| [InlineKeyboardButton::callback(button, callback)]);

    bot.send_message(dialogue.chat_id(), "Оберіть тип тригера:")
        .reply_markup(InlineKeyboardMarkup::new(buttons))
        .await
        .context("Failed to send Telegram message")?;

    dialogue.update(State::ReceiveTriggerType).await?;

    Ok(())
}

pub async fn receive_trigger_type(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
) -> HandlerResult {
    info!("Receiving trigger type...");

    match q.data.as_deref() {
        Some("Lower") => {
            bot.send_message(dialogue.chat_id(), "Вкажіть ціну:")
                .await
                .context("Failed to send Telegram message")?;

            dialogue
                .update(State::ReceivePrice {
                    trigger: Trigger::Lower(0.0),
                })
                .await
                .context("Failed to update state")?;
        }
        Some("Higher") => {
            bot.send_message(dialogue.chat_id(), "Вкажіть ціну:")
                .await
                .context("Failed to send Telegram message")?;

            dialogue
                .update(State::ReceivePrice {
                    trigger: Trigger::Higher(0.0),
                })
                .await
                .context("Failed to update state")?;
        }
        Some(_) | None => {
            bot.send_message(dialogue.chat_id(), "Оберіть одну з доступних опцій")
                .await
                .context("Failed to send Telegram message")?;
        }
    }

    Ok(())
}

pub async fn receive_price(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    mut trigger: Trigger,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> HandlerResult {
    info!("Receiving trigger price...");

    if let Some(Ok(price)) = msg.text().map(|x| x.replace(',', ".").parse::<f64>()) {
        trigger.set(price);
        add(bot, trigger, msg.chat.id, triggers).await?;

        dialogue.exit().await?;
    } else {
        warn!("User provided invalid price: {:?}", msg.text());
        bot.send_message(msg.chat.id, "Вкажіть число:").await?;
    }

    Ok(())
}

pub async fn add(
    bot: Bot,
    trigger: Trigger,
    chat_id: ChatId,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> HandlerResult {
    let mut locked_triggers = triggers.lock().await;

    if locked_triggers
        .entry(chat_id)
        .or_default()
        .iter()
        .any(|x| x == &trigger)
    {
        info!("Trigger {trigger:?} already exists for chat {chat_id}");

        bot.send_message(chat_id, format!("Тригер `{trigger:?}` вже існує"))
            .await
            .context("Failed to send Telegram message")?;

        return Ok(());
    }

    locked_triggers
        .entry(chat_id)
        .or_default()
        .push(trigger.clone());

    locked_triggers
        .entry(chat_id)
        .or_default()
        .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if let Err(err) = backup(&locked_triggers) {
        error!("Failed to backup triggers, due to: {}", err);
    }

    bot.send_message(
        chat_id,
        format!("Вам прийде повідомлення якщо ціна буде {trigger}"),
    )
    .await
    .context("Failed to send Telegram message")?;

    info!("Added {trigger:?} trigger NEAR price for chat {chat_id}");

    Ok(())
}

pub async fn list(
    bot: Bot,
    msg: Message,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> HandlerResult {
    info!("Listing triggers...");

    let mut locked_triggers = triggers.lock().await;

    let mut message = if locked_triggers.is_empty() {
        String::from("У вас наразі немає тригерів")
    } else {
        String::from(
            "Надіслати повідомлення 
якщо ціна буде:\n",
        )
    };

    locked_triggers
        .entry(msg.chat.id)
        .or_default()
        .iter()
        .for_each(|x| message.push_str(format!("{x}\n").as_str()));

    bot.send_message(msg.chat.id, message)
        .await
        .context("Failed to send Telegram message")?;

    Ok(())
}

pub async fn show_trigger_to_delete(
    bot: Bot,
    dialogue: MyDialogue,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> HandlerResult {
    info!("Choosing trigger to delete...");

    let mut buttons = Vec::new();
    triggers
        .lock()
        .await
        .entry(dialogue.chat_id())
        .or_default()
        .iter()
        .for_each(|trigger| {
            buttons.push(InlineKeyboardButton::callback(
                trigger.to_string(),
                trigger.price().to_string(),
            ));
        });

    if buttons.is_empty() {
        bot.send_message(dialogue.chat_id(), "У вас наразі немає тригерів")
            .await?;
        dialogue.exit().await.context("Failed to reset state")?;

        return Ok(());
    }

    bot.send_message(dialogue.chat_id(), "Оберіть тригер для видалення:")
        .reply_markup(InlineKeyboardMarkup::new(vec![buttons]))
        .await
        .context("Failed to send Telegram message")?;

    dialogue
        .update(State::DeleteTrigger)
        .await
        .context("Failed to send update state")?;

    Ok(())
}

pub async fn choose_trigger_to_delete(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> HandlerResult {
    info!("Receiving trigger to delete...");

    match q
        .data
        .as_deref()
        .map(|x| x.replace(',', ".").parse::<f64>())
    {
        Some(Ok(price)) => {
            delete(bot, dialogue.clone(), price, triggers).await?;
            dialogue.exit().await.context("Failed to reset state")?;
        }
        _ => {
            bot.send_message(dialogue.chat_id(), "Оберіть одну з доступних опцій")
                .await
                .context("Failed to send Telegram message")?;
        }
    }

    Ok(())
}

fn remove_triggered(
    triggered: Vec<(ChatId, Trigger)>,
    mut locked_triggers: tokio::sync::MutexGuard<'_, HashMap<ChatId, Vec<Trigger>>>,
) -> bool {
    let mut found = false;

    for (chat_id, trigger) in triggered {
        info!(
            "Removing {:.2}$ from triggers for chat {}",
            trigger.price(),
            chat_id
        );

        locked_triggers
            .entry(chat_id)
            .and_modify(|target_prices| {
                let length_before = target_prices.len();

                target_prices.retain(|trigger_price| match (trigger_price, trigger.clone()) {
                    (Trigger::Lower(x), Trigger::Lower(y))
                    | (Trigger::Higher(x), Trigger::Higher(y)) => (x - y).abs() > f64::EPSILON,
                    (_, Trigger::Neutral(y)) => (trigger_price.price() - y).abs() > f64::EPSILON,
                    _ => true,
                });

                found |= length_before != target_prices.len();
            })
            .or_default();

        if locked_triggers[&chat_id].is_empty() {
            info!("Removing chat {} from triggers", chat_id);

            locked_triggers.remove(&chat_id);
        }
    }

    if let Err(err) = backup(&locked_triggers) {
        error!("Failed to backup triggers, due to: {}", err);
    }

    found
}

pub async fn delete(
    bot: Bot,
    dialogue: MyDialogue,
    price: price::Price,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> HandlerResult {
    info!("Deleting trigger...");

    if remove_triggered(
        vec![(dialogue.chat_id(), Trigger::Neutral(price))],
        triggers.lock().await,
    ) {
        info!("Deleted trigger for chat {}", dialogue.chat_id());

        bot.send_message(
            dialogue.chat_id(),
            format!("Тригер на {price:.2}$ був видалений"),
        )
        .await
        .context("Failed to send Telegram message")?;
    } else {
        info!(
            "No trigger was found to delete for chat {}",
            dialogue.chat_id()
        );

        bot.send_message(
            dialogue.chat_id(),
            format!("Тригер {price:.2}$ не був знайдений"),
        )
        .await
        .context("Failed to send Telegram message")?;
    }

    Ok(())
}

pub async fn delete_all(
    bot: Bot,
    msg: Message,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> HandlerResult {
    info!("Deleting all triggers...");

    let mut locked_triggers = triggers.lock().await;

    if !locked_triggers.contains_key(&msg.chat.id) {
        info!("No triggers were found for chat {}", msg.chat.id);
        bot.send_message(msg.chat.id, "У вас наразі немає тригерів")
            .await
            .context("Failed to send Telegram message")?;

        return Ok(());
    }

    if locked_triggers.contains_key(&msg.chat.id) {
        info!("Deleting all triggers for chat {}", msg.chat.id);
        locked_triggers.remove(&msg.chat.id);
    }

    if let Err(err) = backup(&locked_triggers) {
        error!("Failed to backup triggers, due to: {}", err);
    }

    bot.send_message(msg.chat.id, "Всі тригери були видалені")
        .await
        .context("Failed to send Telegram message")?;

    Ok(())
}

pub async fn process(
    bot: Bot,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
) -> ResponseResult<()> {
    let mut interval = interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        let locked_triggers = triggers.lock().await;
        let mut triggered = Vec::new();

        if !locked_triggers.is_empty() {
            let current_price = price::get().await;

            for (chat_id, triggers_vec) in locked_triggers.iter() {
                if let Ok(price) = current_price {
                    for trigger in triggers_vec {
                        if let &Trigger::Lower(target_price) = trigger {
                            if price <= target_price {
                                info!("NEAR price is lower than {target_price:.2}$ for chat {chat_id}");

                                bot.send_message(
                                    *chat_id,
                                    format!(
                                        "Ціна на NEAR зараз менше ніж {target_price:.2}$\nПоточна ціна: {price:.2}$"
                                    ),
                                )
                                .await?;

                                triggered.push((*chat_id, trigger.clone()));
                            }
                        } else if let &Trigger::Higher(target_price) = trigger {
                            if price >= target_price {
                                info!("NEAR price is higher than {target_price:.2}$ for chat {chat_id}");

                                bot.send_message(
                                    *chat_id,
                                    format!(
                                        "Ціна на NEAR зараз більше ніж {target_price:.2}$\nПоточна ціна: {price:.2}$"
                                    ),
                                )
                                .await?;

                                triggered.push((*chat_id, trigger.clone()));
                            }
                        }
                    }
                }
            }

            if !triggered.is_empty() {
                remove_triggered(triggered, locked_triggers);
            }
        }
    }
}

fn backup(triggers: &HashMap<ChatId, Vec<Trigger>>) -> Result<()> {
    info!("Backing up triggers...");

    let content = serialize(triggers)?;
    write("triggers.bak", content)?;

    Ok(())
}

pub fn restore() -> Result<HashMap<ChatId, Vec<Trigger>>> {
    info!("Restoring triggers...");

    let content = read("triggers.bak")?;
    let triggers = deserialize(&content)?;

    Ok(triggers)
}
