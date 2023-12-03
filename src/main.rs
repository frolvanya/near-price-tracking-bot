use crate::commands::{price, schema, triggers, State};

use anyhow::Result;

use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::*;

mod commands;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    log::info!("Starting near price notifier bot...");

    let bot = Bot::from_env();

    let triggers = match triggers::restore() {
        Ok(triggers) => Arc::new(Mutex::new(triggers)),
        Err(err) => {
            log::error!("Failed to restore triggers: {}", err);
            Arc::new(Mutex::new(HashMap::new()))
        }
    };

    let last_price = Arc::new(Mutex::new(price::Last {
        price: None,
        timestamp: 0,
    }));

    tokio::spawn(triggers::process(
        bot.clone(),
        triggers.clone(),
        last_price.clone(),
    ));

    Dispatcher::builder(bot, schema::process().await)
        .dependencies(dptree::deps![
            InMemStorage::<State>::new(),
            last_price,
            triggers
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
