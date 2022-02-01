use std::{collections::HashMap, env};

use async_recursion::async_recursion;
use serde::{Deserialize, Serialize};
use serde_dynamo::from_item;
use serde_json::{from_value, json, to_value, Value};

use crate::{
    constants::RESULTS_TABLE_ENV,
    get_item_from_table,
    utils::{fetch_utf8_from_vault, json_merge},
    PlaybookArtifacts, ResultsTableItem,
};

const VAULT_TOKEN: &str = "vault:";
const PATH_TOKEN: &str = "$.";
const CONVERSION_TOKEN: &str = "!";

pub async fn resolve_parameters(
    params: &HashMap<String, Value>,
    socless_context: &SoclessContext,
) -> HashMap<String, Value> {
    let mut resolved_parameters = HashMap::new();
    for (parameter, reference) in params {
        resolved_parameters.insert(
            parameter.to_owned(),
            resolve_reference(&reference, socless_context).await,
        );
    }

    resolved_parameters
}

/// The SOCless Event structure required to run a SOCless integration lambda function
/// The Lambda function execution context. The values in this struct
/// are populated using the [Lambda environment variables](https://docs.aws.amazon.com/lambda/latest/dg/current-supported-versions.html)
/// and the headers returned by the poll request to the Runtime APIs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoclessLambdaInput {
    #[serde(rename = "State_Config")]
    pub state_config: StateConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _testing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sfn_context: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<PlaybookArtifacts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Value>,
    #[serde(flatten)]
    pub other: HashMap<String, Value>,
}

impl SoclessLambdaInput {
    pub async fn resolve_state_config_parameters(&mut self, socless_context: &SoclessContext) {
        self.state_config.parameters =
            resolve_parameters(&self.state_config.parameters, socless_context).await;
    }
}

impl From<Value> for SoclessLambdaInput {
    fn from(event: Value) -> Self {
        let mut socless_event: SoclessLambdaInput = match from_value((&event).to_owned()) {
            Ok(correct_event) => correct_event,
            Err(_e) => {
                println!(
                    "Event missing StateConfig, attempting to build Event as direct_invoke mode."
                );
                SoclessLambdaInput {
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
            socless_event = SoclessLambdaInput {
                task_token: Some(token),
                ..from_value(
                    socless_event
                        .sfn_context
                        .expect("'sfn_context' not found in socless event with a 'task_token'"),
                )
                .expect("'sfn_context' object does not deserialize into a SoclessLambdaInput type")
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
    pub name: String,
    #[serde(rename = "Parameters")]
    pub parameters: HashMap<String, Value>,
    #[serde(flatten)]
    pub other: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoclessContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_name: Option<String>,
    #[serde(flatten)]
    pub other: HashMap<String, Value>,
}

/// Evaluate a reference path and return the referenced value
/// ### Example
/// ```
/// # use serde_json::{from_value, json, to_value, Value};
/// # use socless::resolver::{resolve_reference, SoclessContext};
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
            resolved_dict.insert(key.to_owned(), resolve_reference(value, root_obj).await);
        }

        to_value(resolved_dict).unwrap()
    } else if reference_path.is_array() {
        let mut resolved_list: Vec<Value> = vec![];
        for item in reference_path.as_array().unwrap() {
            resolved_list.push(resolve_reference(item, root_obj).await);
        }

        to_value(resolved_list).unwrap()
    } else if reference_path.is_string() {
        let ref_string = reference_path.as_str().unwrap();

        let (trimmed_ref, _conversion) = match ref_string.split_once(CONVERSION_TOKEN) {
            Some((trimmed_ref, conversion)) => (trimmed_ref.to_string(), Some(conversion)),
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
        value_before_convert
    } else {
        reference_path.to_owned()
    }
}

/// Resolves a vault reference to the actual vault file content.
///
/// This handles vault references e.g `vault:file_name` that are passed
/// in as parameters to Socless integrations. It fetches and returns the content
/// of the Vault object with name `file_name` in the vault.
async fn resolve_vault_path(reference_path: &str) -> String {
    let (_, file_id) = reference_path.split_once(VAULT_TOKEN).unwrap();
    let data = fetch_utf8_from_vault(&file_id).await;
    data
}

/// Resolves a JsonPath reference to the actual value referenced.
///
/// reference_path = "$.artifacts.investigation_id"
///
/// Does not support the full JsonPath specification.
async fn resolve_json_path(reference_path: &str, root_obj: &SoclessContext) -> Value {
    let (_pre, post) = reference_path.split_once(PATH_TOKEN).unwrap();

    let mut obj_copy = to_value(root_obj).unwrap();

    for key in post.split('.') {
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

#[cfg(test)]
pub fn mock_event_value_boilerplate() -> Value {
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
                    // "vault.txt": "vault:socless_vault_tests.txt",  // requires mocking s3 vault
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

#[cfg(test)]
pub fn build_mock_root_obj() -> SoclessContext {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn build_mock_event_from_playbook() -> SoclessLambdaInput {
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
    fn build_mock_event_with_references() -> SoclessLambdaInput {
        from_value(mock_event_value_boilerplate()).unwrap()
    }

    #[test]
    fn test_build_socless_event_struct_from_direct_invoke() {
        let mock_event_data = json!({
            "user_id": "U12345",
            "channel_id": "C123458"
        });

        let _test = SoclessLambdaInput::from(mock_event_data);
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
}
