pub mod schema;

pub mod help;
pub mod price;
pub mod triggers;

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
