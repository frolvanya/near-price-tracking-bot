use crate::commands::{Command, HandlerResult};

use anyhow::Context;
use log::info;

use teloxide::{prelude::*, utils::command::BotCommands};

pub async fn process(bot: Bot, msg: Message) -> HandlerResult {
    info!("Receiving help command...");

    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await
        .context("Failed to send Telegram message")?;

    Ok(())
}
