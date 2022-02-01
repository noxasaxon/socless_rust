use crate::clients::get_or_init_dynamo;
use crate::constants::RESULTS_TABLE_ENV;
use crate::resolver::{SoclessContext, SoclessLambdaInput};
use crate::utils::{fetch_utf8_from_vault, get_item_from_table, json_merge};
use crate::{PlaybookArtifacts, ResultsTableItem};
use async_recursion::async_recursion;
use lambda_runtime::Context;
use serde::{Deserialize, Serialize};
use serde_dynamo::{from_item, to_attribute_value};
use serde_json::{from_value, json, to_value, Value};
use std::env;
use std::future::Future;
use std::{collections::HashMap, env::var};

async fn build_socless_context(event: &SoclessLambdaInput) -> SoclessContext {
    let temp_event = event.clone();
    let is_testing = temp_event._testing.unwrap_or(false);

    let socless_context: SoclessContext = match is_testing {
        true => from_value(to_value(&temp_event).unwrap()).unwrap(),
        false => {
            let execution_id = &temp_event.execution_id.unwrap();
            let item_response: ResultsTableItem = from_item(
                get_item_from_table(
                    "execution_id",
                    execution_id,
                    &env::var(RESULTS_TABLE_ENV).unwrap(),
                )
                .await
                .expect("Execution ID not found in Results Table"),
            )
            .unwrap();

            let mut temp_ctx = json!(&item_response.results);
            json_merge(
                &mut temp_ctx,
                json!({
                    "execution_id" : &execution_id,
                    "errors" : &temp_event.errors,
                }),
            );
            if (&temp_event.task_token.is_some()).to_owned() {
                json_merge(
                    &mut temp_ctx,
                    json!({
                    "task_token" : &temp_event.task_token,
                    "state_name" : &temp_event.state_config.name}),
                );
            };

            from_value(temp_ctx).unwrap()
        }
    };
    socless_context
}

////! converting to json might be automatic with serde
// fn apply_conversion(value_to_convert: Value, conversion_key: &str) -> Value {
//     if conversion_key == "json" {
//         serde_json::from_str
//     }
//     json!({})
// }

/// Take an AWS lambda Event (serde Value) and Context, map it to SOCless execution global state,
/// trigger the integration handler function using a resolved event with global state,
/// and save the results of that execution back to the global state.
/// # Example
///
/// ```ignore
/// use socless::socless_bootstrap;
/// ```
pub async fn socless_bootstrap<Fut>(
    event: Value,
    _context: Context,
    handler: fn(Value) -> Fut,
    include_event: bool,
) -> Value
where
    Fut: Future<Output = Value>,
{
    let mut socless_event = SoclessLambdaInput::from(event);

    let socless_context = build_socless_context(&socless_event).await;

    socless_event
        .resolve_state_config_parameters(&socless_context)
        .await;

    let mut event_params = socless_event.state_config.parameters.clone();

    if include_event {
        event_params.insert(
            "context".to_owned(),
            to_value(socless_context.to_owned()).unwrap(),
        );
    }

    let handler_result = handler(
        to_value(&event_params).expect("Unable to serialize event_params hashmap to serde Value."),
    )
    .await;

    if !handler_result.is_object() {
        panic!("output returned from the integration handler is not a json map object.")
    }

    if !&socless_event._testing.unwrap_or_default() {
        save_state_results(
            &socless_event.state_config.name,
            &socless_event
                .execution_id
                .expect("No execution_id in non-testing event"),
            &handler_result,
            socless_context.errors,
        )
        .await;
    }
    handler_result
}

/// Save the results of a State's execution to the Execution results table
pub async fn save_state_results(
    state_config_name: &str,
    execution_id: &str,
    handler_result: &Value,
    // socless_context: &SoclessContext,
    socless_context_errors: Option<HashMap<String, Value>>,
) {
    let mut update_item = get_or_init_dynamo()
        .await
        .update_item()
        .table_name(
            var(RESULTS_TABLE_ENV)
                .expect("No Environment Variable set for 'SOCLESS_RESULTS_TABLE'"),
        )
        .key("execution_id", to_attribute_value(execution_id).unwrap())
        .expression_attribute_names("#name", state_config_name)
        .expression_attribute_names("#last_results", "_Last_Saved_Results")
        .expression_attribute_values(
            ":r",
            to_attribute_value(handler_result)
                .expect("Unable to convert 'handler_result' to AttributeValue for PutItem"),
        );

    update_item = if let Some(context_errors_map) = socless_context_errors {
        update_item
            .expression_attribute_values(
                ":e",
                to_attribute_value(context_errors_map)
                    .expect("Unable to convert 'errors' to AttributeValue for PutItem"),
            )
            .update_expression(
                "SET #results.#results.#name = :r, #results.#results.#last_results = :r ,#results.errors = :e",
            )
    } else {
        update_item.update_expression(
            "SET #results.#results.#name = :r, #results.#results.#last_results = :r ",
        )
    };
    update_item
        .send()
        .await
        .expect("Unable to save result to Results Table");
}

#[cfg(test)]
mod tests {
    use crate::resolver::{
        build_mock_root_obj, mock_event_value_boilerplate, SoclessContext, SoclessLambdaInput,
    };

    use super::*;

    #[tokio::test]
    async fn test_build_socless_boilerplate_with_complete_event_already_set_up() {
        let event_with_state_config = SoclessLambdaInput::from(mock_event_value_boilerplate());
        assert_eq!(
            to_value(event_with_state_config).unwrap(),
            mock_event_value_boilerplate()
        );
    }

    #[tokio::test]
    async fn test_resolve_state_config_parameters() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();
        let mut event_with_state_config = SoclessLambdaInput::from(mock_event_value_boilerplate());

        event_with_state_config
            .resolve_state_config_parameters(&mock_root_obj)
            .await;

        let resolved_params_as_value =
            to_value(event_with_state_config.state_config.parameters).unwrap();
        let expected = json!(
            {
                "firstname": "Sterling",
                "lastname": "Archer",
                "middlename": "Malory",
                "acquaintances": [{"firstname": "Malory", "lastname": "Archer"}]
            }
        );

        assert_eq!(resolved_params_as_value, expected);
    }

    // #[tokio::test]
    // async fn test_build_state_config() {
    //     let mock_root_obj: SoclessContext = build_mock_root_obj();
    //     let event_with_state_config =
    //         build_socless_event_boilerplate(mock_event_value_boilerplate(), Some(false)).await;
    //     let event = resolve_state_config_parameters(&event_with_state_config, &mock_root_obj).await;

    //     println!("{}", event);

    //     let expected = to_value(json!(
    //         {
    //             "firstname": "Sterling",
    //             "lastname": "Archer",
    //             "middlename": "Malory",
    //             "acquaintances": [{"firstname": "Malory", "lastname": "Archer"}]
    //         }
    //     ))
    //     .unwrap();

    //     assert_eq!(event["State_Config"]["Parameters"], expected);
    // }

    // #[tokio::test]
    // async fn test_resolve_state_config_parameters_resolve_parameters() {
    //     let mock_root_obj: SoclessContext = build_mock_root_obj();
    //     let mock_event = build_mock_event_with_references();
    //     let event = resolve_state_config_parameters(&mock_event, &mock_root_obj).await;

    //     println!("{}", event);

    //     let expected = to_value(json!(
    //         {
    //             "firstname": "Sterling",
    //             "lastname": "Archer",
    //             "middlename": "Malory",
    //             "acquaintances": [{"firstname": "Malory", "lastname": "Archer"}]
    //         }
    //     ))
    //     .unwrap();

    //     assert_eq!(event["State_Config"]["Parameters"], expected);
    // }

    // #[tokio::test]
    // async fn test_resolve_jsonpath_vault_token() {
    //     let mock_root_obj: SoclessContext = from_value(json!({
    //         "artifacts": {
    //             "event": {
    //                 "details": {
    //                     "firstname": "Sterling",
    //                     "middlename": "Malory",
    //                     "lastname": "Archer",
    //                     "vault_test" : "vault:socless_vault_tests.txt"
    //                 }
    //             }
    //         }
    //     }))
    //     .unwrap();

    //     let result = resolve_json_path("$.artifacts.event.details.firstname", &mock_root_obj).await;
    //     assert_eq!(
    //         result,
    //         to_value(mock_root_obj).unwrap()["artifacts"]["event"]["details"]["firstname"]
    //     );
    // }
}
