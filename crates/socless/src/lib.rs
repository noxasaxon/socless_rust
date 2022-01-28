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
pub mod events;
pub mod helpers;
pub mod humaninteraction;
pub mod integrations;
pub mod models;
pub mod utils;

pub use events::{create_events, SoclessEventBatch};
pub use helpers::{
    fetch_utf8_from_vault, get_dynamo_client, get_item_from_table, get_object_from_s3,
    get_s3_client, json_merge, put_item_in_table, split_with_delimiter, update_item_in_table,
};
pub use humaninteraction::{end_human_interaction, init_human_interaction};
pub use integrations::{socless_bootstrap, SoclessContext, SoclessLambdaInput, StateConfig};
pub use models::{
    EventTableItem, PlaybookArtifacts, PlaybookInput, ResponsesTableItem, ResultsTableItem,
    SoclessEvent,
};
pub use utils::{gen_datetimenow, gen_id};
