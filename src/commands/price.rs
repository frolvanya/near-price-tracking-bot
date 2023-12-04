use crate::commands::HandlerResult;

use anyhow::Result;
use log::info;

use teloxide::prelude::*;

use binance::api::Binance;
use binance::market::Market;

pub type Price = f64;

pub fn get() -> Result<Price, anyhow::Error> {
    info!("Getting NEAR price...");

    let market: Market = Binance::new(None, None);

    match market.get_price("NEARUSDT") {
        Ok(symbol_price) => Ok(symbol_price.price),
        Err(err) => Err(anyhow::anyhow!(
            "Error, while parsing crypto price: {:?}",
            err
        )),
    }
}

pub async fn process(bot: Bot, msg: Message) -> HandlerResult {
    info!("Getting NEAR price...");

    let price = match get() {
        Ok(price) => price,
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

    Ok(())
}
