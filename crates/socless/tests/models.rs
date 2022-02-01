use serde::{Deserialize, Serialize};
use serde_json::Value;
use socless::{PlaybookArtifacts, PlaybookInput};

#[derive(Serialize, Deserialize, Default)]
pub struct IntegrationTestDefinition {
    pub name: String,
    pub events_db_before_test: PlaybookInput,
    pub lambda_input_before_resolve: Option<Value>,
    pub lambda_input_after_resolve: Option<Value>,
    /// If not present, don't check the DB (nothing was saved to check)
    pub events_db_after_test: Option<Value>,
}

impl IntegrationTestDefinition {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            events_db_before_test: PlaybookInput {
                artifacts: PlaybookArtifacts {
                    event: socless::EventTableItem {
                        id: name.to_string(),
                        ..Default::default()
                    },
                    execution_id: name.to_string(),
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
