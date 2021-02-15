use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SoclessEvent {
    pub id: String,
    pub investigation_id: String,
    pub status_: String,
    pub is_duplicate: bool,
    pub created_at: String,
    pub event_type: String,
    pub playbook: String,
    pub details: HashMap<String, Value>, // single dict with unknown types
    pub data_types: HashMap<String, String>,
    pub event_meta: HashMap<String, String>,
    pub dedup_keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ResultsTableItem {
    pub execution_id: String,
    pub investigation_id: String,
    pub datetime: String,
    pub results: PlaybookInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaybookInput {
    pub artifacts: PlaybookArtifacts,
    pub results: HashMap<String, Value>,
    pub errors: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaybookArtifacts {
    pub event: EventTableItem,
    pub execution_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventTableItem {
    // no dedup_keys
    pub id: String,
    pub investigation_id: String,
    pub status_: String,
    pub is_duplicate: bool,
    pub created_at: String,
    pub event_type: String,
    pub playbook: String,
    pub details: HashMap<String, Value>, // single dict with unknown types
    pub data_types: HashMap<String, String>,
    pub event_meta: HashMap<String, String>,
}

impl From<SoclessEvent> for EventTableItem {
    fn from(event: SoclessEvent) -> Self {
        EventTableItem {
            id: event.id,
            investigation_id: event.investigation_id,
            status_: event.status_,
            is_duplicate: event.is_duplicate,
            created_at: event.created_at,
            event_type: event.event_type,
            playbook: event.playbook,
            details: event.details,
            data_types: event.data_types,
            event_meta: event.event_meta,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResponsesTableItem {
    pub message_id: String,
    pub datetime: String,
    pub message: String,
    pub fulfilled: bool,
    pub execution_id: String,
    pub investigation_id: String,
    pub receiver: String,
    pub await_token: String,
}
