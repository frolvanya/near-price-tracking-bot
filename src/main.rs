use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use log::info;
use serde_json::Value;
use tokio::{
    sync::Mutex,
    time::{sleep, Duration},
};

use teloxide::{prelude::*, utils::command::BotCommands};

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
    #[command(description = "send notification when NEAR price is >= value")]
    Track(f64),
}

async fn get_price() -> Result<Option<f64>> {
    let response =
        reqwest::get("https://api.coingecko.com/api/v3/simple/price?ids=near&vs_currencies=usd")
            .await?;
    let body = response.text().await?;
    let parsed_body: Value = serde_json::from_str(&body)?;

    Ok(parsed_body["near"]["usd"].as_f64())
}

async fn track_prices(
    bot: Bot,
    tracked_prices: Arc<Mutex<HashMap<ChatId, Vec<f64>>>>,
) -> Result<()> {
    let interval = Duration::from_secs(5);

    loop {
        let mut locked_tracked_prices = tracked_prices.lock().await;
        let mut removed = Vec::new();

        for (chat_id, target_prices) in locked_tracked_prices.iter() {
            if let Ok(Some(current_price)) = get_price().await {
                for &target_price in target_prices {
                    if current_price >= target_price {
                        info!("NEAR reached {}$ for chat {}", target_price, chat_id);

                        bot.send_message(*chat_id, format!("NEAR reached {target_price}$"))
                            .await?;

                        removed.push((*chat_id, target_price));
                    }
                }
            }
        }

        for (chat_id, target_price) in removed {
            info!(
                "Removing {}$ from tracked prices for chat {}",
                target_price, chat_id
            );

            locked_tracked_prices
                .entry(chat_id)
                .and_modify(|target_prices| {
                    target_prices.retain(|&x| (x - target_price).abs() > f64::EPSILON);
                })
                .or_default();

            if locked_tracked_prices.entry(chat_id).or_default().is_empty() {
                info!("Removing chat {} from tracked prices", chat_id);

                locked_tracked_prices.remove(&chat_id);
            }
        }

        sleep(interval).await;
    }
}

async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    tracked_prices: Arc<Mutex<HashMap<ChatId, Vec<f64>>>>,
) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::GetPrice => {
            info!("Getting NEAR price...");

            let price = match get_price().await {
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

            bot.send_message(msg.chat.id, format!("Current NEAR price: {price}$."))
                .await?;
        }
        Command::Track(price) => {
            info!("Tracking NEAR price >= {}...", price);

            let mut locked_tracked_prices = tracked_prices.lock().await;

            locked_tracked_prices
                .entry(msg.chat.id)
                .or_default()
                .push(price);

            locked_tracked_prices
                .entry(msg.chat.id)
                .or_default()
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            info!("Added {price}$ to tracked prices for chat {}", msg.chat.id);

            bot.send_message(msg.chat.id, format!("Tracking NEAR price >= {price}$."))
                .await?;
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    log::info!("Starting near price tracking bot...");

    let bot = Bot::from_env();
    let tracked_prices = Arc::new(Mutex::new(HashMap::new()));

    tokio::spawn(track_prices(bot.clone(), Arc::clone(&tracked_prices)));

    Command::repl(bot, move |bot, msg, cmd| {
        answer(bot, msg, cmd, Arc::clone(&tracked_prices))
    })
    .await;

    Ok(())
}
