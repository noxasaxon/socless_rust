//! SOCless core library, for Rust!
//!
//! Allows users to write SOCless integrations (lambda functions) in Rust instead of Python.
//!
//! Rust lambda functions have a very short init time, while Python can suffer slow 'cold starts' which can cause time-sensitive operations to fail if cold-starts are not mitigated with cloudwatch schedules or reserved concurrency.

//! What is SOCless?
//! [SOCless](https://twilio-labs.github.io/socless/) is an Automation Framework that provides a global state
//! wrapper and helper utilities around AWS Step Functions.
//!
//! SOCless allows users to write complex State Machines that can do more than pass a Step's
//! output directly to the next step.
pub mod clients;
pub mod constants;
pub mod events;
pub mod humaninteraction;
pub mod integrations;
pub mod models;
pub mod resolver;
pub mod utils;

pub use clients::*;
pub use events::{create_events, SoclessEventBatch};
pub use humaninteraction::{end_human_interaction, init_human_interaction};
pub use integrations::socless_bootstrap;
pub use models::{
    EventTableItem, PlaybookArtifacts, PlaybookInput, ResponsesTableItem, ResultsTableItem,
    SoclessEvent,
};
pub use resolver::{SoclessContext, SoclessLambdaInput, StateConfig};
pub use utils::{gen_datetimenow, gen_id, get_item_from_table};
