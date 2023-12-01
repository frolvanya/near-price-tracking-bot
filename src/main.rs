use anyhow::Result;
use log::info;
use serde_json::Value;

use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use teloxide::{prelude::*, utils::command::BotCommands};

type Price = f64;

#[derive(PartialOrd, PartialEq)]
enum Trigger {
    Lower(Price),
    Higher(Price),
}

struct LastPrice {
    price: Option<Price>,
    timestamp: i64,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "display this text")]
    Help,

    #[command(description = "get current NEAR price")]
    GetPrice,

    #[command(description = "notifies when NEAR price is <= current price")]
    TriggerLower(Price),
    #[command(description = "notifies when NEAR price is >= current price")]
    TriggerHigher(Price),
    #[command(description = "list all my triggers")]
    Triggers,

    #[command(description = "delete trigger for lower price")]
    DeleteLower(Price),
    #[command(description = "delete trigger for higher price")]
    DeleteHigher(Price),
    #[command(description = "delete triggers for lower AND higher prices")]
    Delete(Price),
    #[command(description = "delete ALL triggers")]
    DeleteAll,
}

async fn get_price(last_price: &Arc<Mutex<LastPrice>>) -> Result<Option<Price>> {
    let current_timestamp = chrono::Utc::now().timestamp();

    let mut locked_last_price = last_price.lock().await;
    if locked_last_price.timestamp + 60 > current_timestamp {
        if let Some(price) = locked_last_price.price {
            return Ok(Some(price));
        }
    }

    for _ in 0..10 {
        let response = reqwest::get(
            "https://api.coingecko.com/api/v3/simple/price?ids=near&vs_currencies=usd",
        )
        .await?;

        let body = response.text().await?;
        let parsed_body: Value = serde_json::from_str(&body)?;

        let price = parsed_body["near"]["usd"].as_f64();

        if let Some(price) = price {
            locked_last_price.price = Some(price);
            locked_last_price.timestamp = current_timestamp;
            return Ok(Some(price));
        }
    }

    Err(anyhow::anyhow!("Failed to get NEAR price"))
}

async fn trigger(
    bot: Bot,
    trigger_prices: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
    last_price: Arc<Mutex<LastPrice>>,
) -> Result<()> {
    loop {
        let mut locked_trigger_prices = trigger_prices.lock().await;
        let mut removed = Vec::new();

        if !locked_trigger_prices.is_empty() {
            let current_price = get_price(&last_price).await;

            for (chat_id, target_prices) in locked_trigger_prices.iter() {
                if let Ok(Some(current_price)) = current_price {
                    for target_price in target_prices {
                        if let &Trigger::Lower(target_price) = target_price {
                            if current_price <= target_price {
                                info!("NEAR is lower than {target_price:.2}$ for chat {chat_id}");
                                bot.send_message(
                                    *chat_id,
                                    format!(
                                        "NEAR price is lower than {target_price:.2}$\nCurrent price: {current_price:.2}$"
                                    ),
                                )
                                .await?;
                                removed.push((*chat_id, target_price));
                            }
                        } else if let &Trigger::Higher(target_price) = target_price {
                            if current_price >= target_price {
                                info!("NEAR is higher than {target_price:.2}$ for chat {chat_id}");
                                bot.send_message(
                                    *chat_id,
                                    format!(
                                        "NEAR price is higher than {target_price:.2}$\nCurrent price: {current_price:.2}$"
                                    ),
                                )
                                .await?;
                                removed.push((*chat_id, target_price));
                            }
                        }
                    }
                }
            }
        }

        for (chat_id, target_price) in removed {
            info!(
                "Removing {:.2}$ from trigger prices for chat {}",
                target_price, chat_id
            );

            locked_trigger_prices
                .entry(chat_id)
                .and_modify(|target_prices| {
                    target_prices.retain(|x| {
                        if let Trigger::Lower(x) = x {
                            (x - target_price).abs() > f64::EPSILON
                        } else if let Trigger::Higher(x) = x {
                            (x - target_price).abs() > f64::EPSILON
                        } else {
                            true
                        }
                    });
                })
                .or_default();

            if locked_trigger_prices.entry(chat_id).or_default().is_empty() {
                info!("Removing chat {} from trigger prices", chat_id);

                locked_trigger_prices.remove(&chat_id);
            }
        }
    }
}

async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    trigger_prices: Arc<Mutex<HashMap<ChatId, Vec<Trigger>>>>,
    last_price: Arc<Mutex<LastPrice>>,
) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::GetPrice => {
            info!("Getting NEAR price...");

            let price = match get_price(&last_price).await {
                Ok(price) => {
                    if let Some(price) = price {
                        price
                    } else {
                        bot.send_message(msg.chat.id, "Failed to parse NEAR price")
                            .await?;

                        return Ok(());
                    }
                }
                Err(err) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Failed to get NEAR price, due to: {err}"),
                    )
                    .await?;

                    return Ok(());
                }
            };

            bot.send_message(msg.chat.id, format!("Current NEAR price: {price:.2}$."))
                .await?;
        }
        Command::TriggerLower(price) => {
            info!("Tracking NEAR price <= {price:.2}...");

            let mut locked_trigger_prices = trigger_prices.lock().await;

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .push(Trigger::Lower(price));

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            info!(
                "Added Lower({price:.2})$ to trigger prices for chat {}",
                msg.chat.id
            );

            bot.send_message(msg.chat.id, format!("Tracking NEAR price <= {price:.2}$."))
                .await?;
        }
        Command::TriggerHigher(price) => {
            info!("Tracking NEAR price >= {price:.2}...");

            let mut locked_trigger_prices = trigger_prices.lock().await;

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .push(Trigger::Higher(price));

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            info!(
                "Added Higher({price:.2})$ to trigger prices for chat {}",
                msg.chat.id
            );

            bot.send_message(msg.chat.id, format!("Tracking NEAR price >= {price:.2}$."))
                .await?;
        }
        Command::Triggers => {
            info!("Listing trigger NEAR prices...");

            let mut locked_trigger_prices = trigger_prices.lock().await;

            let mut message = String::new();

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .iter()
                .for_each(|x| {
                    if let Trigger::Lower(x) = x {
                        message.push_str(&format!("Lower({x:.2})$\n"));
                    } else if let Trigger::Higher(x) = x {
                        message.push_str(&format!("Higher({x:.2})$\n"));
                    }
                });

            if message.is_empty() {
                message.push_str("You don't have any triggers");
            }

            bot.send_message(msg.chat.id, message).await?;
        }
        Command::DeleteLower(price) => {
            info!("Deleting trigger NEAR price <= {price:.2}...");

            let mut locked_trigger_prices = trigger_prices.lock().await;
            let mut found = false;

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .retain(|x| {
                    if let Trigger::Lower(x) = x {
                        found = true;
                        (x - price).abs() > f64::EPSILON
                    } else {
                        true
                    }
                });

            if found {
                info!(
                    "Deleted Lower({price:.2})$ from trigger prices for chat {}",
                    msg.chat.id
                );

                bot.send_message(
                    msg.chat.id,
                    format!("Deleted trigger NEAR price <= {price:.2}$."),
                )
                .await?;
            } else {
                info!(
                    "No Lower({price:.2})$ found in trigger prices for chat {}",
                    msg.chat.id
                );

                bot.send_message(
                    msg.chat.id,
                    format!("No trigger NEAR price <= {price:.2}$ found."),
                )
                .await?;
            }
        }
        Command::DeleteHigher(price) => {
            info!("Deleting trigger NEAR price >= {price:.2}...");

            let mut locked_trigger_prices = trigger_prices.lock().await;
            let mut found = false;

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .retain(|x| {
                    if let Trigger::Higher(x) = x {
                        found = true;
                        (x - price).abs() > f64::EPSILON
                    } else {
                        true
                    }
                });

            if found {
                info!(
                    "Deleted Higher({price:.2})$ from trigger prices for chat {}",
                    msg.chat.id
                );

                bot.send_message(
                    msg.chat.id,
                    format!("Deleted trigger NEAR price >= {price:.2}$."),
                )
                .await?;
            } else {
                info!(
                    "No Higher({price:.2})$ found in trigger prices for chat {}",
                    msg.chat.id
                );

                bot.send_message(
                    msg.chat.id,
                    format!("No trigger NEAR price >= {price:.2}$ found."),
                )
                .await?;
            }
        }
        Command::Delete(price) => {
            info!("Delete trigger NEAR price <= {price:.2} and >= {price:.2}...");

            let mut locked_trigger_prices = trigger_prices.lock().await;
            let mut found = false;

            locked_trigger_prices
                .entry(msg.chat.id)
                .or_default()
                .retain(|x| {
                    if let Trigger::Lower(x) = x {
                        found = true;
                        (x - price).abs() > f64::EPSILON
                    } else if let Trigger::Higher(x) = x {
                        found = true;
                        (x - price).abs() > f64::EPSILON
                    } else {
                        true
                    }
                });

            if found {
                info!(
                    "Deleted Lower({price:.2})$ and Higher({price:.2})$ from trigger prices for chat {}",
                     msg.chat.id
                );

                bot.send_message(
                    msg.chat.id,
                    format!("Deleted trigger NEAR price <= {price:.2}$ and >= {price:.2}$."),
                )
                .await?;
            } else {
                info!(
                    "No Lower({price:.2})$ and Higher({price:.2}) found in trigger prices for chat {}",
                    msg.chat.id
                );

                bot.send_message(
                    msg.chat.id,
                    format!("No trigger NEAR price <= {price:.2}$ or >= {price:.2}$ found."),
                )
                .await?;
            }
        }
        Command::DeleteAll => {
            info!("Deleting all trigger NEAR prices...");

            let mut locked_trigger_prices = trigger_prices.lock().await;
            locked_trigger_prices.remove(&msg.chat.id);

            info!("Deleted all trigger NEAR prices for chat {}", msg.chat.id);

            bot.send_message(msg.chat.id, "Deleted all trigger NEAR prices.")
                .await?;
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    log::info!("Starting near price notifier bot...");

    let bot = Bot::from_env();

    let trigger_prices = Arc::new(Mutex::new(HashMap::new()));
    let last_price = Arc::new(Mutex::new(LastPrice {
        price: None,
        timestamp: 0,
    }));

    tokio::spawn(trigger(
        bot.clone(),
        Arc::clone(&trigger_prices),
        Arc::clone(&last_price),
    ));

    Command::repl(bot, move |bot, msg, cmd| {
        answer(
            bot,
            msg,
            cmd,
            Arc::clone(&trigger_prices),
            Arc::clone(&last_price),
        )
    })
    .await;

    Ok(())
}
