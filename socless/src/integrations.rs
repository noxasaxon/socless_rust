/// Compare to https://github.com/twilio-labs/socless_python/blob/master/socless/integrations.py
use crate::{
    events::{PlaybookArtifacts, ResultsTableItem},
    fetch_utf8_from_vault, get_item_from_table, json_merge, split_with_delimiter,
    update_item_in_table,
};
use async_recursion::async_recursion;
use lamedh_runtime::Context;
use rusoto_dynamodb::{AttributeValue, UpdateItemInput};
use serde::{Deserialize, Serialize};
use serde_dynamo::{from_item, to_attribute_value};
use serde_json::{from_value, json, to_value, Value};
use std::{collections::HashMap, env::var};

const VAULT_TOKEN: &str = "vault:";
const PATH_TOKEN: &str = "$.";
const CONVERSION_TOKEN: &str = "!";

/// The SOCless Event structure required to run a SOCless integration lambda function
/// The Lambda function execution context. The values in this struct
/// are populated using the [Lambda environment variables](https://docs.aws.amazon.com/lambda/latest/dg/current-supported-versions.html)
/// and the headers returned by the poll request to the Runtime APIs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoclessLambdaEvent {
    #[serde(rename = "State_Config")]
    state_config: StateConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    _testing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sfn_context: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts: Option<PlaybookArtifacts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Value>,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

impl SoclessLambdaEvent {
    async fn resolve_state_config_parameters(&mut self, socless_context: &SoclessContext) {
        let current_state_config = self.clone().state_config;

        let mut resolved_state_config = StateConfig {
            parameters: HashMap::new(),
            ..current_state_config
        };
        for (parameter, reference) in current_state_config.parameters {
            resolved_state_config.parameters.insert(
                parameter,
                resolve_reference(&reference, socless_context).await,
            );
        }

        self.state_config = resolved_state_config
    }
}

impl From<Value> for SoclessLambdaEvent {
    fn from(event: Value) -> Self {
        let mut socless_event: SoclessLambdaEvent = match from_value((&event).to_owned()) {
            Ok(correct_event) => correct_event,
            Err(_e) => {
                println!(
                    "Event missing StateConfig, attempting to build Event as direct_invoke mode."
                );
                SoclessLambdaEvent {
                    state_config: StateConfig {
                        name: "direct_invoke".to_string(),
                        parameters: from_value(event)
                            .expect("unable to convert entire event to 'Parameters' hashmap"),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }
        };

        if let Some(token) = socless_event.task_token {
            socless_event = SoclessLambdaEvent {
                task_token: Some(token),
                ..from_value(
                    socless_event
                        .sfn_context
                        .expect("'sfn_context' not found in socless event with a 'task_token'"),
                )
                .expect("'sfn_context' object does not deserialize into a SoclessLambdaEvent type")
            }
        }

        if socless_event.execution_id.is_none() && socless_event.artifacts.is_none() {
            println!(
                "No State_Config was passed to the integration, likely due to invocation \
            from outside of a SOCless playbook. Running this lambda in test mode."
            );
            socless_event._testing = Some(true);
            ////! might be unnecessary now if we build StateConfig as direct_invoke at the beginning of this function
            // socless_event.state_config = StateConfig {
            //     name: "direct_invoke".to_string(),
            //     parameters: socless_event.clone().other,
            //     other: HashMap::new(),
            // };
        }

        socless_event
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateConfig {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Parameters")]
    parameters: HashMap<String, Value>,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoclessContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    results: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_name: Option<String>,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

async fn build_socless_context(event: &SoclessLambdaEvent) -> SoclessContext {
    let temp_event = event.clone();
    let is_testing = temp_event._testing.unwrap_or(false);

    let socless_context: SoclessContext = match is_testing {
        true => from_value(to_value(&temp_event).unwrap()).unwrap(),
        false => {
            let execution_id = &temp_event.execution_id.unwrap();
            let item_response: ResultsTableItem = from_item(
                get_item_from_table(
                    "execution_id",
                    &execution_id,
                    &var("SOCLESS_RESULTS_TABLE").unwrap(),
                )
                .await
                .unwrap(),
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

            from_value(to_value(temp_ctx).unwrap()).unwrap()
        }
    };
    socless_context
}

/// Evaluate a reference path and return the referenced value
/// ```
/// # use serde_json::{from_value, json, to_value, Value};
/// # use socless::integrations::{resolve_reference, SoclessContext};
/// let root_object: SoclessContext = from_value(json!({
///     "artifacts": {
///         "event": {
///             "details": {
///                 "firstname": "Sterling",
///                 "lastname": "Archer",
///             }
///         }
///     }
/// })).unwrap();
/// let result =
/// # tokio_test::block_on(
/// resolve_reference(&json!([{"firstname": "$.artifacts.event.details.firstname"}, "$.artifacts.event.details.lastname"]), &root_object)
/// );
/// let expected_result = json!([{"firstname": "Sterling"}, "Archer"]);
/// assert_eq!(result, expected_result);
/// ```
#[async_recursion]
pub async fn resolve_reference(reference_path: &Value, root_obj: &SoclessContext) -> Value {
    if reference_path.is_object() {
        let mut resolved_dict: HashMap<String, Value> = HashMap::new();
        for (key, value) in reference_path.as_object().unwrap() {
            resolved_dict.insert(key.to_owned(), resolve_reference(&value, root_obj).await);
        }
        return to_value(resolved_dict).unwrap();
    } else if reference_path.is_array() {
        let mut resolved_list: Vec<Value> = vec![];
        for item in reference_path.as_array().unwrap() {
            resolved_list.push(resolve_reference(item, root_obj).await);
        }
        return to_value(resolved_list).unwrap();
    } else if reference_path.is_string() {
        let ref_string = reference_path.as_str().unwrap();

        // let (trimmed_ref, _conversion) = match ref_string.split_with_delimiter(CONVERSION_TOKEN) {
        let (trimmed_ref, _conversion) = match split_with_delimiter(ref_string, CONVERSION_TOKEN) {
            Some((trimmed_ref, _, conversion)) => (trimmed_ref.to_string(), Some(conversion)),
            None => (ref_string.to_string(), None),
        };

        let value_before_convert = if trimmed_ref.starts_with(VAULT_TOKEN) {
            to_value(resolve_vault_path(&trimmed_ref).await).unwrap()
        } else if trimmed_ref.starts_with(PATH_TOKEN) {
            resolve_json_path(&trimmed_ref, root_obj).await
        } else {
            to_value(trimmed_ref).unwrap()
        };

        ////! only json conversions currently supported and I THINK serde_json is automatically doing that
        // return match _conversion {
        //     Some(conversion_key) => apply_conversion(value_before_convert, conversion_key),
        //     None => value_before_convert,
        // };
        return value_before_convert;
    } else {
        return reference_path.to_owned();
    }
}

/// Resolves a vault reference to the actual vault file content.
///
/// This handles vault references e.g `vault:file_name` that are passed
/// in as parameters to Socless integrations. It fetches and returns the content
/// of the Vault object with name `file_name` in the vault.
async fn resolve_vault_path(reference_path: &str) -> String {
    let (_, _, file_id) = split_with_delimiter(reference_path, VAULT_TOKEN).unwrap();
    let data = fetch_utf8_from_vault(&file_id).await;
    data
}

/// Resolves a JsonPath reference to the actual value referenced.
///
/// reference_path = "$.artifacts.investigation_id"
///
/// Does not support the full JsonPath specification.
async fn resolve_json_path(reference_path: &str, root_obj: &SoclessContext) -> Value {
    let (_pre, _, post) = split_with_delimiter(reference_path, PATH_TOKEN).unwrap();

    let mut obj_copy = to_value(root_obj).unwrap();

    for key in post.split(".") {
        let mut value = obj_copy[key].to_owned();
        if value.is_null() {
            panic!(
                "Unable to resolve key {}, parent object does not exist. Full path: {}",
                key, reference_path
            );
        } else {
            if let Some(string_value) = value.as_str() {
                if string_value.starts_with(VAULT_TOKEN) {
                    value = to_value(resolve_vault_path(string_value).await).unwrap();
                }
            }
            obj_copy = value;
        }
    }
    obj_copy
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
pub async fn socless_bootstrap(
    event: Value,
    _context: Context,
    handler: fn(Value) -> Value,
    include_event: bool,
) -> Value {
    // let mut socless_event: SoclessLambdaEvent = build_socless_event_boilerplate(event);
    let mut socless_event = SoclessLambdaEvent::from(event);

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
    );

    if !handler_result.is_object() {
        panic!("output returned from the integration handler is not a json map object.")
    }

    if !&socless_event._testing.unwrap_or_default() {
        save_state_results(&socless_event, &handler_result, &socless_context).await;
    }
    return handler_result;
}

/// Save the results of a State's execution to the Execution results table
async fn save_state_results(
    socless_event: &SoclessLambdaEvent,
    handler_result: &Value,
    socless_context: &SoclessContext,
) {
    let mut expression_attribute_names: HashMap<String, String> = HashMap::new();
    let mut expression_attribute_values: HashMap<String, AttributeValue> = HashMap::new();

    expression_attribute_values.insert(
        ":r".to_owned(),
        to_attribute_value(handler_result)
            .expect("Unable to convert 'handler_result' to AttributeValue for PutItem"),
    );

    let errors: HashMap<String, Value> = socless_context.errors.clone().unwrap_or_default();
    let error_expression = match errors.is_empty() {
        true => "",
        false => {
            expression_attribute_values.insert(
                ":e".to_owned(),
                to_attribute_value(errors)
                    .expect("Unable to convert 'errors' to AttributeValue for PutItem"),
            );
            ",#results.errors = :e"
        }
    };

    let update_expression = format!(
        "SET #results.#results.#name = :r, #results.#results.#last_results = :r {}",
        error_expression
    );

    expression_attribute_names.insert("#name".to_string(), socless_event.state_config.name.clone());
    expression_attribute_names.insert(
        "#last_results".to_string(),
        "_Last_Saved_Results".to_string(),
    );

    let mut key: HashMap<String, AttributeValue> = HashMap::new();
    key.insert(
        "execution_id".to_string(),
        to_attribute_value(socless_event.execution_id.clone().unwrap()).unwrap(),
    );

    let input = UpdateItemInput {
        table_name: var("SOCLESS_RESULTS_TABLE")
            .expect("No Environment Variable set for 'SOCLESS_RESULTS_TABLE'"),
        key,
        update_expression: Some(update_expression),
        expression_attribute_names: Some(expression_attribute_names),
        expression_attribute_values: Some(expression_attribute_values),
        ..Default::default()
    };

    update_item_in_table(input)
        .await
        .expect("Unable to save result to Results Table");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_event_value_boilerplate() -> Value {
        json!({
                "execution_id": "98123-1234567",
                "artifacts": {
                    "event": {
                        "id": "1234-45678-abcd",
                        "created_at": "2021-01-16T00:57:06.573112Z",
                        "data_types": {},
                        "details": {
                            "firstname": "Sterling",
                            "middlename": "Malory",
                            "lastname": "Archer",
                            "a_map": {
                                "jfutz" : "littleboyblew"
                            },
                            // "vault_test" : "vault:socless_vault_tests.txt"
                        },
                        "event_type": "mock_test_event",
                        "event_meta": {},
                        "investigation_id": "1234-45678-abcd",
                        "status_": "open",
                        "is_duplicate": false,
                        "playbook": "MockTestPlaybook"
                    },
                    "execution_id": "98123-1234567"
                },
                "State_Config": {
                    "Name": "Authenticate_User",
                    "Parameters": {
                        "firstname": "$.artifacts.event.details.firstname",
                        "lastname": "$.artifacts.event.details.lastname",
                        "middlename": "Malory",
                        // "vault.txt": "vault:socless_vault_tests.txt",
                        // "vault.json": "vault:socless_vault_tests.json!json",
                        "acquaintances": [
                            {
                                "firstname": "$.artifacts.event.details.middlename",
                                "lastname": "$.artifacts.event.details.lastname"
                            }
                        ]
                    }
                }
        })
    }

    fn build_mock_root_obj() -> SoclessContext {
        from_value(json!({
            "artifacts": {
                "event": {
                    "details": {
                        "firstname": "Sterling",
                        "middlename": "Malory",
                        "lastname": "Archer",
                        "a_map": {
                            "jfutz" : "littleboyblew"
                        },
                        "vault_test" : "vault:socless_vault_tests.txt"
                    }
                }
            }
        }))
        .unwrap()
    }

    #[allow(dead_code)]
    fn build_mock_event_from_playbook() -> SoclessLambdaEvent {
        let mock_event_data = json!({
            "execution_id": "98123-1234567",
            "artifacts": {
                "event": {
                    "id": "1234-45678-abcd",
                    "created_at": "2021-01-16T00:57:06.573112Z",
                    "data_types": {},
                    "details": {
                        "user_id" : "W1234999",
                        "channel_id": "C123456",
                        "channel_name": "testing-dev",
                    },
                    "event_type": "mock_test_event",
                    "event_meta": {},
                    "investigation_id": "1234-45678-abcd",
                    "status_": "open",
                    "is_duplicate": false,
                    "playbook": "MockTestPlaybook"
                },
                "execution_id": "98123-1234567"
            },
            "State_Config": {
                "Name": "Authenticate_User",
                "Parameters": {
                    "user_id": "$.artifacts.event.details.user_id",
                    "channel_id": "$.artifacts.event.details.channel_id"
                }
            }
        });
        from_value(mock_event_data).unwrap()
    }

    #[allow(dead_code)]
    fn build_mock_event_with_references() -> SoclessLambdaEvent {
        from_value(mock_event_value_boilerplate()).unwrap()
    }

    #[test]
    fn test_build_socless_event_struct_from_direct_invoke() {
        let mock_event_data = json!({
            "user_id": "U12345",
            "channel_id": "C123458"
        });

        SoclessLambdaEvent::from(mock_event_data);
    }

    #[tokio::test]
    async fn test_resolve_jsonpath_string() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();

        let result = resolve_json_path("$.artifacts.event.details.firstname", &mock_root_obj).await;
        assert_eq!(
            result,
            to_value(mock_root_obj).unwrap()["artifacts"]["event"]["details"]["firstname"]
        );
    }

    #[tokio::test]
    async fn test_resolve_jsonpath_map() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();

        let result = resolve_json_path("$.artifacts.event.details.a_map", &mock_root_obj).await;
        assert_eq!(
            result,
            to_value(mock_root_obj).unwrap()["artifacts"]["event"]["details"]["a_map"]
        );
    }

    #[tokio::test]
    async fn test_resolve_reference_string_passthrough() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();

        let result = resolve_reference(&to_value("hello").unwrap(), &mock_root_obj).await;

        assert_eq!(result, to_value("hello").unwrap());
    }

    #[tokio::test]
    async fn test_resolve_reference_jsonpath_map() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();

        let result = resolve_reference(
            &to_value("$.artifacts.event.details.a_map").unwrap(),
            &mock_root_obj,
        )
        .await;

        assert_eq!(
            result,
            to_value(mock_root_obj).unwrap()["artifacts"]["event"]["details"]["a_map"]
        );
    }

    #[tokio::test]
    async fn test_resolve_reference_jsonpath_map_2() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();

        let result = resolve_reference(
            &json!({"firstname": "$.artifacts.event.details.firstname"}),
            &mock_root_obj,
        )
        .await;

        assert_eq!(result, json!({"firstname": "Sterling"}));
    }

    #[tokio::test]
    async fn test_resolve_reference_array_passthrough() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();

        let result = resolve_reference(&json!(["test"]), &mock_root_obj).await;

        assert_eq!(result, json!(["test"]));
    }

    #[tokio::test]
    async fn test_resolve_reference_jsonpath_nested_array() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();

        let result = resolve_reference(&json!([{"firstname": "$.artifacts.event.details.firstname"}, "$.artifacts.event.details.lastname"]), &mock_root_obj).await;

        assert_eq!(result, json!([{"firstname": "Sterling"}, "Archer"]));
    }

    #[tokio::test]
    async fn test_build_socless_boilerplate_with_complete_event_already_set_up() {
        let event_with_state_config = SoclessLambdaEvent::from(mock_event_value_boilerplate());
        assert_eq!(
            to_value(event_with_state_config).unwrap(),
            mock_event_value_boilerplate()
        );
    }

    #[tokio::test]
    async fn test_resolve_state_config_parameters() {
        let mock_root_obj: SoclessContext = build_mock_root_obj();
        let mut event_with_state_config = SoclessLambdaEvent::from(mock_event_value_boilerplate());

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
