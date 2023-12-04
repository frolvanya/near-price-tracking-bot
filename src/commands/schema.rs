use crate::commands::{help, price, triggers, Command, MyDialogue, State};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use dptree::case;
use teloxide::{
    dispatching::{dialogue, dialogue::InMemStorage, UpdateHandler},
    prelude::*,
};

pub async fn process() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    let command_handler = teloxide::filter_command::<Command, _>().branch(
        case![State::Start]
            .branch(case![Command::Help].endpoint(help::process))
            .branch(case![Command::GetPrice].endpoint(price::process))
            .branch(case![Command::AddTrigger].endpoint(triggers::start))
            .branch(case![Command::DeleteTrigger].endpoint(triggers::show_trigger_to_delete))
            .branch(case![Command::DeleteAll].endpoint(triggers::delete_all))
            .branch(case![Command::ListTriggers].endpoint(triggers::list)),
    );

    let message_handler = Update::filter_message().branch(command_handler).branch(
        case![State::ReceivePrice { trigger }].endpoint(
            |bot: Bot,
             dialogue: MyDialogue,
             msg: Message,
             trigger: triggers::Trigger,
             triggers: Arc<Mutex<HashMap<ChatId, Vec<triggers::Trigger>>>>| {
                triggers::receive_price(bot, dialogue, msg, trigger, triggers)
            },
        ),
    );

    let callback_query_handler = Update::filter_callback_query()
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
