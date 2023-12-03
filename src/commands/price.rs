use crate::commands::HandlerResult;

use anyhow::Result;
use log::info;
use serde_json::Value;

use std::sync::Arc;
use tokio::sync::Mutex;

use teloxide::prelude::*;

pub type Price = f64;

pub struct Last {
    pub price: Option<Price>,
    pub timestamp: i64,
}

pub async fn get(last_price: &Arc<Mutex<Last>>) -> Result<Option<Price>> {
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

pub async fn process(bot: Bot, msg: Message, last_price: Arc<Mutex<Last>>) -> HandlerResult {
    info!("Getting NEAR price...");

    let price = match get(&last_price).await {
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

    Ok(())
}
