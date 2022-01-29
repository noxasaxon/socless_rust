use std::{collections::HashMap, env::var};

use serde_dynamo::{from_item, to_attribute_value, to_item};
use serde_json::{from_value, to_string, to_value, Value};

use maplit::hashmap;

use crate::{
    clients::{get_or_init_dynamo, get_or_init_sfn},
    gen_datetimenow, gen_id, get_item_from_table,
    integrations::save_state_results,
    ResponsesTableItem, ResultsTableItem, SoclessContext,
};

/// Initialize the human interaction worfklow by saving the Human Interaction Task Token to SOCless Message Responses Table.
///
///  `execution_context` (dict): The playbook execution context object that contains the task token
///
///  `message_draft` (string):  The message you intend to send. This will be stored in alongside the task token in the SOCless
///         Message Responses Table for record keeping purposes. You still have to send the message yourself in your integration
///
///  `message_id` (string): The ID to use to track both the interaction request and the human's response
///
/// _RETURNS_: A `message_id` to embed in your message such that is returned as part of the human's response.
/// It serves as a call_back ID to help SOCless match the users response to the right playbook execution
pub async fn init_human_interaction<'a>(
    execution_context: SoclessContext,
    message_draft: &str,
    message_id: Option<String>,
) -> String {
    let resolved_msg_id = message_id.unwrap_or(gen_id());

    let investigation_id: String =
        from_value(execution_context.artifacts.unwrap()["event"]["investigation_id"].clone())
            .unwrap();

    let response_table_item = ResponsesTableItem {
        investigation_id,
        message_id: resolved_msg_id.clone(),
        datetime: gen_datetimenow(),
        message: message_draft.to_owned(),
        fulfilled: false,
        execution_id: execution_context
            .execution_id
            .expect("No execution id found in context"),
        receiver: execution_context
            .state_name
            .expect("No `state_name` found in context"),
        await_token: execution_context
            .task_token
            .expect("No `await_token` found in context"),
    };

    let result = get_or_init_dynamo()
        .await
        .put_item()
        .table_name(
            &var("SOCLESS_MESSAGE_RESPONSE_TABLE").expect("No env var set for response table"),
        )
        .set_item(Some(
            to_item(&response_table_item).expect("unable to convert to item"),
        ))
        .send()
        .await
        .unwrap();

    return resolved_msg_id;
}

/// Completes a human interaction by returning the human's response to
/// the appropriate playbook execution
///
/// message_id (str): The ID in the human's response that identifies the interaction
///
/// response_body (dict): The human's response
pub async fn end_human_interaction(message_id: String, response_body: Value) {
    let response_table_name =
        var("SOCLESS_MESSAGE_RESPONSE_TABLE").expect("No env var set for response table");

    let response_table_item = get_item_from_table("message_id", &message_id, &response_table_name)
        .await
        .expect("message_id not found in Response Table");

    let response: ResponsesTableItem = from_item(response_table_item)
        .expect("Unable to deserialize ResponseTableItem, malformed table item");

    if response.fulfilled {
        panic!(
            "Message ID {} for end_human_interaction already used",
            message_id
        )
    }

    let results_item = get_item_from_table(
        &var("SOCLESS_RESULTS_TABLE").unwrap(),
        "execution_id",
        &response.execution_id,
    )
    .await
    .expect("execution_id not found in Results Table");

    let results_table_item: ResultsTableItem = from_item(results_item)
        .expect("Unable to deserialize to ResultsTableItem, malformed table item");

    let mut execution_results = results_table_item.results;

    let response_body_as_hashmap: HashMap<String, Value> =
        from_value(response_body.clone()).expect("response_body not a <String, Value> type");

    execution_results.results = hashmap! {
        response.receiver.to_owned() => response_body.to_owned(),
    };

    execution_results.results.extend(response_body_as_hashmap);

    save_state_results(
        &response.receiver.to_string(),
        &response.execution_id,
        &response_body,
        None,
    )
    .await;

    get_or_init_sfn()
        .await
        .send_task_success()
        .task_token(response.await_token)
        .output(
            to_string(&execution_results)
                .expect("Unable to convert PlaybookInput `execution_results` to json string"),
        )
        .send()
        .await
        .expect("step_functions.send_task_success failed");

    get_or_init_dynamo()
        .await
        .update_item()
        .table_name(response_table_name)
        .key("message_id", to_attribute_value(message_id).unwrap())
        .update_expression(
            "SET fulfilled = :fulfilled, response_payload = :response_payload".to_string(),
        )
        .expression_attribute_values(
            ":fulfilled",
            to_attribute_value(true).expect("Error converting to ExpressionAttributeValue"),
        )
        .expression_attribute_values(
            ":response_payload",
            to_attribute_value(response_body)
                .expect("Error converting to ExpressionAttributeValue"),
        )
        .send()
        .await
        .expect("Unable to save result to Results Table");
}
