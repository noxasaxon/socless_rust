use aws_sdk_sfn::input::StartExecutionInput;
// compare to https://github.com/twilio-labs/socless_python/blob/master/socless/events.py
use lambda_http::Context;
use md5;
// use rusoto_core::Region;
// use rusoto_dynamodb::PutItemInput;
// use rusoto_stepfunctions::{StartExecutionInput, StepFunctions, StepFunctionsClient};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::collections::HashMap;
use std::env;

use crate::{
    clients::{get_or_init_dynamo, get_or_init_sfn},
    gen_datetimenow, gen_id,
    helpers::get_item_from_table,
    EventTableItem, PlaybookArtifacts, PlaybookInput, ResultsTableItem, SoclessEvent,
};

use aws_sdk_dynamodb::{
    input::PutItemInput,
    model::{AttributeValue, DeleteRequest, KeysAndAttributes, PutRequest, WriteRequest},
};
use serde_dynamo::aws_sdk_dynamodb_0_4::{from_item, from_items, to_attribute_value, to_item};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SoclessEventBatch {
    pub created_at: Option<String>,
    pub event_type: String,
    pub playbook: String,
    pub details: Vec<Value>, // list of dicts with unknown types
    pub data_types: Option<HashMap<String, String>>,
    pub event_meta: Option<HashMap<String, String>>,
    pub dedup_keys: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ExecutionStatus {
    pub status: bool,
    pub message: Value,
}

pub async fn create_events(
    event_batch: SoclessEventBatch,
    lambda_context: lambda_http::Context,
) -> Vec<ExecutionStatus> {
    println!("lambda context: {:?}", lambda_context);
    let mut execution_statuses: Vec<ExecutionStatus> = vec![];

    let playbook = &event_batch.playbook.to_owned();

    let formatted_events = setup_events(event_batch);

    let playbook_arn = get_playbook_arn(playbook, &lambda_context);

    let events_table_name = std::env::var("SOCLESS_EVENTS_TABLE")
        .expect("No env var found for SOCLESS_EVENTS_TABLE, please check serverless.yml");

    let mut events_subset: Vec<EventTableItem> = vec![];
    for event in formatted_events {
        let deduplicated = deduplicate(event).await;

        let event_table_input = EventTableItem::from(deduplicated);

        let result = get_or_init_dynamo()
            .await
            .put_item()
            .table_name(&events_table_name)
            .set_item(Some(
                to_item(&event_table_input).expect("unable to convert to item"),
            ))
            .send()
            .await
            .unwrap();

        events_subset.push(event_table_input);
    }

    for creation_event in events_subset {
        execution_statuses.push(execute_playbook(creation_event, &playbook_arn).await);
    }

    execution_statuses
}

fn setup_events(events_batch: SoclessEventBatch) -> Vec<SoclessEvent> {
    let mut formatted_events = vec![];

    let created_at = events_batch.created_at.unwrap_or(gen_datetimenow());

    for event_details in events_batch.details {
        let investigation_id = gen_id();

        let new_event = SoclessEvent {
            id: investigation_id.to_owned(),
            investigation_id,
            status_: "open".to_string(),
            is_duplicate: false,
            created_at: created_at.to_owned(),
            event_type: events_batch.event_type.to_owned(),
            playbook: events_batch.playbook.to_owned(),
            details: serde_json::from_value(event_details).unwrap(),
            data_types: events_batch.data_types.clone().unwrap_or_default(),
            event_meta: events_batch.event_meta.clone().unwrap_or_default(),
            dedup_keys: events_batch.dedup_keys.clone().unwrap_or_default(),
        };

        formatted_events.push(new_event);
    }

    formatted_events
}

fn build_dedup_hash(event: &SoclessEvent) -> String {
    let dedup_kv_pairs: Vec<(String, Value)> = event
        .details
        .clone()
        .into_iter()
        .filter(|x| event.dedup_keys.clone().into_iter().any(|y| y == x.0))
        .collect();

    let mut sorted_dedup_values: Vec<String> = vec![];
    for kv_pair in dedup_kv_pairs {
        sorted_dedup_values.push(kv_pair.0);
    }
    sorted_dedup_values.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    let dedup_signature: String = format!(
        "{}{}",
        event.event_type.to_lowercase(),
        sorted_dedup_values.join("")
    );

    let dedup_hash = format!("{:x}", md5::compute(dedup_signature));
    dedup_hash
}

async fn deduplicate(mut event: SoclessEvent) -> SoclessEvent {
    // let cached_dedup_hash = dedup_hash.clone();

    let dedup_hash = build_dedup_hash(&event);

    // get dedup_table item
    ////! not implemented, TODO FIX
    let dedup_mapping: HashMap<String, Value> = HashMap::new();
    let possible_investigation_id = dedup_mapping.get("current_investigation_id");

    match possible_investigation_id {
        None => println!(
            "unmapped dedup_hash detected in dedup table: {}",
            // json!({ "dedup_hash": cached_dedup_hash })
            json!({ "dedup_hash": dedup_hash })
        ),
        Some(inv_id) => {
            let current_investigation_id = inv_id.to_string();

            let events_table_name = std::env::var("SOCLESS_EVENTS_TABLE")
                .expect("No env var found for SOCLESS_EVENTS_TABLE, please check serverless.yml");

            let possible_existing_event =
                get_item_from_table("id", &current_investigation_id, &events_table_name).await;

            match possible_existing_event {
                Some(item) => {
                    let existing_event: EventTableItem = from_item(item).unwrap();
                    if existing_event.status_ != "closed" {
                        event.status_ = "closed".to_string();
                        event.investigation_id = existing_event.investigation_id;
                        event.is_duplicate = true;
                    }
                }
                None => println!(
                    "No existing investigation found for current_investigation_id: {}",
                    &current_investigation_id
                ),
            }
        }
    };

    event
}

async fn execute_playbook(creation_event: EventTableItem, playbook_arn: &str) -> ExecutionStatus {
    let execution_id = gen_id();
    let investigation_id = creation_event.investigation_id.clone();

    // make playbook artifacts
    let playbook_artifacts = PlaybookArtifacts {
        event: creation_event,
        execution_id: execution_id.clone(),
    };

    let playbook_input = PlaybookInput {
        artifacts: playbook_artifacts,
        results: HashMap::new(),
        errors: HashMap::new(),
    };

    let results_table_input = ResultsTableItem {
        execution_id: execution_id.clone(),
        datetime: gen_datetimenow(),
        investigation_id: investigation_id.to_owned(),
        results: playbook_input.clone(),
    };

    let results_table_name =
        env::var("SOCLESS_RESULTS_TABLE").expect("SOCLESS_RESULTS_TABLE not set in env!");

    let result = get_or_init_dynamo()
        .await
        .put_item()
        .table_name(&results_table_name)
        .set_item(Some(
            to_item(&results_table_input).expect("unable to convert to item"),
        ))
        .send()
        .await
        .unwrap();

    let start_exec_response = get_or_init_sfn()
        .await
        .start_execution()
        .name(&execution_id)
        .state_machine_arn(playbook_arn)
        .input(
            json!({"execution_id": execution_id, "artifacts": playbook_input.artifacts})
                .to_string(),
        )
        .send()
        .await;

    // get_or_init_sfn().await.start_execution().input(

    // );

    return match start_exec_response {
        Ok(start_exec_output) => ExecutionStatus {
            status: true,
            message: json!({
                "execution_id" : start_exec_output.execution_arn,
                "investigation_id" : investigation_id
            }),
        },
        Err(error) => ExecutionStatus {
            status: false,
            message: json!({ "error": format!("Error during State Machine Start: {}", error) }),
        },
    };
}

fn get_playbook_arn(playbook_name: &str, lambda_context: &Context) -> String {
    let lambda_arn_split = lambda_context
        .invoked_function_arn
        .split(":")
        .collect::<Vec<&str>>();
    let region = lambda_arn_split[3];
    let account_id = lambda_arn_split[4];

    format!(
        "arn:aws:states:{}:{}:stateMachine:{}",
        region, account_id, playbook_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    // use lamedh_http::lambda::Config;
    use lambda_http::lambda_runtime::Config;

    #[test]
    fn test_results_table_struct() {
        let mock_event_data = json!({
            "datetime": "2021-02-02T16:19:53.032610Z",
            "execution_id": "12345-asdf-1234",
            "investigation_id": "987654-98765",
            "results": {
                "artifacts": {
                    "event": {
                    "created_at": "2021-02-02T16:19:52.567359Z",
                    "data_types": {},
                    "details": {
                        "assignee": "jfutz",
                        "attachment_name": "test_attachment.txt",
                    },
                    "event_meta": {},
                    "event_type": "mock_testing_event",
                    "id": "987654-98765",
                    "investigation_id": "987654-98765",
                    "is_duplicate": false,
                    "playbook": "MockTestingPlaybook",
                    "status_": "open"
                    },
                    "execution_id": "12345-asdf-1234"
                },
                "errors": {},
                "results": {
                "_Last_Saved_Results": {
                    "result": "success"
                },
                "Add_Comment": {
                    "author": {
                    "active": true,
                    "displayName": "jfutz",
                    "emailAddress": "littleboyblew@kronish.com",
                    "key": "sumthin",
                    "name": "sumthin",
                    "timeZone": "America/Los_Angeles"
                    },
                    "body": "this is an added comment",
                    "created": "2021-02-02T16:20:17.747+0000",
                },
                "Add_Labels": {
                    "status": "success"
                },
                "Assign_Ticket": {
                    "status": "success",
                    "username": "jfutz"
                }
                }
            }
        });
        use serde_json::from_value;
        let _mock_results_table_item: ResultsTableItem = from_value(mock_event_data).unwrap();
    }

    #[test]
    fn test_get_playbook_arn() {
        let mut mock_context = Context::default();
        mock_context.request_id = "5bd30a31-e89d-46de-84ec-cc0a5089962c".to_string();
        mock_context.deadline = 1609836879440;
        mock_context.invoked_function_arn = "arn:aws:lambda:us-west-2:12345678901:function:_socless_rust_create_events_slash_command".to_string();
        mock_context.env_config = Config {
            endpoint: "127.0.0.1:1234".to_string(),
            function_name: "_socless_rust_create_events_slash_command".to_string(),
            memory: 128,
            version: "$LATEST".to_string(),
            log_stream: "2021/01/05/[$LATEST]12345678".to_string(),
            log_group: "/aws/lambda/_socless_rust_create_events_slash_command".to_string(),
            ..Default::default()
        };

        assert_eq!(
            &get_playbook_arn("testing_playbook", &mock_context),
            "arn:aws:states:us-west-2:12345678901:stateMachine:testing_playbook"
        );
    }

    #[test]
    fn test_build_dedup_hash() {
        let details: HashMap<String, Value> = serde_json::from_value(json!({
        "api_app_id": "A1234567",
        "channel_id": "G123456",
        "channel_name": "privategroup",
        "command": "/update_config",
        "enterprise_id": "EF1234567",
        "enterprise_name": "A-Company",
        "response_url": "https://hooks.slack.com/commands/T1234567/123456678/alsdfjlasjf",
        "team_domain": "test_domain",
        "team_id": "T123456",
        "text": "testing123",
        "trigger_id": "123456789.123456789.b11d2434423456789",
        "user_id": "W12345",
        "user_name": "shunt"
        }))
        .unwrap();

        let mock_socless_event = SoclessEvent {
            id: "005366e8-c64a-4587-af8e-343d5775d3b3".to_string(),
            investigation_id: "005366e8-c64a-4587-af8e-343d5775d3b3".to_string(),
            status_: "open".to_string(),
            is_duplicate: false,
            created_at: "2020-11-24T08:52:27.916090Z".to_string(),
            event_type: "SoclessUtilsIntegrationTest".to_string(),
            playbook: "SoclessUtilsIntegrationTest".to_string(),
            details: details,
            data_types: HashMap::new(),
            event_meta: HashMap::new(),
            dedup_keys: vec!["trigger_id".to_string()],
        };

        let dedup_hash = build_dedup_hash(&mock_socless_event);

        assert_eq!("3dc424cb39725b818a72b796d7a64376".to_string(), dedup_hash);
    }
}
