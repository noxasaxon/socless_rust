use crate::clients::get_or_init_dynamo;
use aws_sdk_dynamodb::{error::PutItemError, model::AttributeValue, output::PutItemOutput};
use chrono::Utc;
use serde_dynamo::{to_attribute_value, to_item};
use std::collections::HashMap;
use uuid::Uuid;

/// Generate current timestamp in ISO8601 UTC format
/// # Example
/// ```
/// use socless::gen_datetimenow;
/// println!("{}", gen_datetimenow());
/// ```
pub fn gen_datetimenow() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

/// Generate a uuid (used for execution and investigation ids)
/// # Example
/// ```
/// use socless::gen_id;
/// println!("{}", gen_id());
/// ```
pub fn gen_id() -> String {
    Uuid::new_v4().to_string()
}

pub async fn get_item_from_table(
    primary_key_name: &str,
    primary_key_value: &str,
    table_name: &str,
) -> Option<HashMap<String, AttributeValue>> {
    let client = get_or_init_dynamo().await;

    let result = client
        .get_item()
        .key(
            primary_key_name,
            to_attribute_value(primary_key_value).unwrap(),
        )
        .send()
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Error in get_item of table: {} for key= {{ {} : {} }}",
                table_name, primary_key_name, primary_key_value
            )
        });

    result.item
}

/// ## Example
/// ```ignore
/// put_item_in_table(&results_table_name, &results_table_input)
/// .await
/// .expect("failed to store item");
/// ```
pub async fn put_item_in_table(
    table_name: &str,
    table_item: impl serde::ser::Serialize,
) -> Result<PutItemOutput, aws_sdk_dynamodb::SdkError<PutItemError>> {
    get_or_init_dynamo()
        .await
        .put_item()
        .table_name(table_name)
        .set_item(Some(to_item(table_item).unwrap()))
        .send()
        .await
}

pub async fn update_item_in_table(
    table_name: &str,
    table_item: impl serde::ser::Serialize,
) -> Result<PutItemOutput, aws_sdk_dynamodb::SdkError<PutItemError>> {
    get_or_init_dynamo()
        .await
        .put_item()
        .table_name(table_name)
        .set_item(Some(to_item(table_item).unwrap()))
        .send()
        .await
}

use crate::clients::get_or_init_s3;
use aws_sdk_s3::{error::GetObjectError, output::GetObjectOutput};
use serde_json::Value;
use std::env::var;

/// Combine two serde Value objects
/// # Example
///```
/// use serde_json::json;
/// use socless::utils::json_merge;
///
/// let mut mutable_json_object = json!({ "foo" : "bar" });
/// let object_to_merge = json!({ "baz" : "spam" });
/// json_merge(&mut mutable_json_object, object_to_merge);
/// assert_eq!(mutable_json_object, json!({"foo" : "bar", "baz" : "spam"}));
///```
pub fn json_merge(a: &mut Value, b: Value) {
    if let Value::Object(a) = a {
        if let Value::Object(b) = b {
            for (k, v) in b {
                if v.is_null() {
                    a.remove(&k);
                } else {
                    json_merge(a.entry(k).or_insert(Value::Null), v);
                }
            }
            return;
        }
    }
    *a = b;
}

pub async fn get_object_from_s3(
    key: &str,
    bucket_name: &str,
) -> Result<GetObjectOutput, aws_sdk_s3::SdkError<GetObjectError>> {
    get_or_init_s3()
        .await
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await
}

pub async fn fetch_utf8_from_vault(key: &str) -> String {
    let socless_vault_bucket_name: String =
        var(&"SOCLESS_VAULT").expect("No env var found for SOCLESS_VAULT s3 bucket");

    let object_result = get_object_from_s3(key, &socless_vault_bucket_name).await;
    let object = object_result.unwrap_or_else(|_| panic!("No object found for key: {}", key));

    let body_as_bytes = object.body.collect().await.unwrap().into_bytes();

    String::from_utf8(body_as_bytes.to_vec()).expect("S3 file is not valid utf8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_id() {
        assert_eq!(36, gen_id().len());
    }

    #[test]
    fn test_gen_datetimenow() {
        assert_eq!(27, gen_datetimenow().len());
    }
}
