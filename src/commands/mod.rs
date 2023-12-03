pub mod schema;

pub mod help;
pub mod price;
pub mod triggers;

use serde::{Deserialize, Serialize};
use std::fmt;

use teloxide::utils::command::BotCommands;
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(PartialOrd, PartialEq, Clone, Serialize, Deserialize)]
pub enum Trigger {
    Lower(price::Price),
    Higher(price::Price),
}

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceiveTriggerType,
    ReceivePrice {
        trigger: Trigger,
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

impl Trigger {
    fn set(&mut self, price: price::Price) {
        match self {
            Self::Lower(x) | Self::Higher(x) => *x = price,
        }
    }

    const fn price(&self) -> price::Price {
        match self {
            Self::Lower(x) | Self::Higher(x) => *x,
        }
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lower(x) => write!(f, "менша за {x:.2}$"),
            Self::Higher(x) => write!(f, "більша за {x:.2}$"),
        }
    }
}

impl fmt::Debug for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lower(x) => write!(f, "Trigger::Lower({x:.2})"),
            Self::Higher(x) => write!(f, "Trigger::Higher({x:.2})"),
        }
    }
}
