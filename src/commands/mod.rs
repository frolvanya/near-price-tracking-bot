pub mod schema;

pub mod help;
pub mod price;
pub mod triggers;

use anyhow::Context;
use log::warn;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use teloxide::utils::command::BotCommands;
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceiveTriggerType,
    ReceivePrice {
        trigger: triggers::Trigger,
    },
    DeleteTrigger,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "display this text")]
    Help,

    #[command(description = "get current NEAR price")]
    GetPrice,

    #[command(description = "add new trigger")]
    AddTrigger,
    #[command(description = "delete selected trigger")]
    DeleteTrigger,
    #[command(description = "delete all triggers")]
    DeleteAll,
    #[command(description = "list all my triggers")]
    ListTriggers,
}

pub async fn start(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
    triggers: Arc<Mutex<HashMap<ChatId, Vec<triggers::Trigger>>>>,
) -> HandlerResult {
    if let Some(data) = q.data.clone() {
        if data == "Lower" || data == "Higher" {
            triggers::receive_trigger_type(bot, dialogue, q).await?;
        } else if data.parse::<price::Price>().is_ok() {
            triggers::choose_trigger_to_delete(bot, dialogue, q, triggers).await?;
        } else {
            warn!("Unknown callback query data: {}", data);

            bot.send_message(dialogue.chat_id(), "Невідома команда")
                .await
                .context("Failed to send Telegram message")?;
        }
    }

    Ok(())
}
