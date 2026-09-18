use crate::commands::{help, price, start, triggers, Command, MyDialogue, State};

use anyhow::Context;
use log::warn;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use dptree::case;
use teloxide::{
    dispatching::{dialogue, dialogue::InMemStorage, UpdateHandler},
    prelude::*,
};

pub fn process() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    // Commands are handled in every state: otherwise a dialogue left in
    // `ReceiveTriggerType` or `DeleteTrigger` swallows every command silently.
    // Commands that do not set a state of their own reset the dialogue first.
    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::Help].endpoint(
            |bot: Bot, dialogue: MyDialogue, msg: Message| async move {
                dialogue.exit().await.context("Failed to reset state")?;
                help::process(bot, msg).await
            },
        ))
        .branch(case![Command::GetPrice].endpoint(
            |bot: Bot, dialogue: MyDialogue, msg: Message| async move {
                dialogue.exit().await.context("Failed to reset state")?;
                price::process(bot, msg).await
            },
        ))
        .branch(case![Command::AddTrigger].endpoint(triggers::start))
        .branch(case![Command::DeleteTrigger].endpoint(triggers::show_trigger_to_delete))
        .branch(case![Command::DeleteAll].endpoint(
            |bot: Bot,
             dialogue: MyDialogue,
             msg: Message,
             triggers: Arc<Mutex<HashMap<ChatId, Vec<triggers::Trigger>>>>| async move {
                dialogue.exit().await.context("Failed to reset state")?;
                triggers::delete_all(bot, msg, triggers).await
            },
        ))
        .branch(case![Command::ListTriggers].endpoint(
            |bot: Bot,
             dialogue: MyDialogue,
             msg: Message,
             triggers: Arc<Mutex<HashMap<ChatId, Vec<triggers::Trigger>>>>| async move {
                dialogue.exit().await.context("Failed to reset state")?;
                triggers::list(bot, msg, triggers).await
            },
        ));

    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(case![State::ReceivePrice { trigger }].endpoint(
            |bot: Bot,
             dialogue: MyDialogue,
             msg: Message,
             trigger: triggers::Trigger,
             triggers: Arc<Mutex<HashMap<ChatId, Vec<triggers::Trigger>>>>| {
                triggers::receive_price(bot, dialogue, msg, trigger, triggers)
            },
        ))
        // Nothing matched: answer instead of dropping the update silently.
        .branch(dptree::endpoint(|bot: Bot, msg: Message| async move {
            warn!("Unhandled message: {:?}", msg.text());

            bot.send_message(msg.chat.id, "Невідома команда. Скористайтесь /help")
                .await
                .context("Failed to send Telegram message")?;

            Ok(())
        }));

    let callback_query_handler = Update::filter_callback_query()
        .branch(case![State::Start].endpoint(start))
        .branch(case![State::ReceiveTriggerType].endpoint(triggers::receive_trigger_type))
        .branch(case![State::DeleteTrigger].endpoint(
            |bot: Bot,
             dialogue: MyDialogue,
             q: CallbackQuery,
             triggers: Arc<Mutex<HashMap<ChatId, Vec<triggers::Trigger>>>>| {
                triggers::choose_trigger_to_delete(bot, dialogue, q, triggers)
            },
        ));

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
        .branch(callback_query_handler)
}
