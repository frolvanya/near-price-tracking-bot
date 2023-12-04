use crate::commands::HandlerResult;

use anyhow::{anyhow, Context, Result};
use log::{error, info};

use teloxide::prelude::*;

use binance::api::Binance;
use binance::market::Market;

pub type Price = f64;

pub async fn get() -> Result<Price> {
    let result = tokio::task::spawn_blocking(move || {
        let market: Market = Binance::new(None, None);
        market.get_price("NEARUSDT")
    })
    .await
    .context("Failed to spawn blocking task")?;

    match result {
        Ok(symbol_price) => Ok(symbol_price.price),
        Err(err) => {
            error!("Failed to get NEAR price: {err}");
            Err(anyhow!("Error while parsing NEAR price: {:?}", err))
        }
    }
}

pub async fn process(bot: Bot, msg: Message) -> HandlerResult {
    info!("Getting NEAR price...");

    let price = match get().await {
        Ok(price) => price,
        Err(err) => {
            bot.send_message(
                msg.chat.id,
                format!("Failed to get NEAR price, due to: {err}"),
            )
            .await
            .context("Failed to send Telegram message")?;

            return Ok(());
        }
    };

    bot.send_message(msg.chat.id, format!("Current NEAR price: {price:.2}$."))
        .await
        .context("Failed to send Telegram message")?;

    Ok(())
}
