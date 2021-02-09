use rusoto_core::{Region, RusotoError};
use rusoto_dynamodb::{
    AttributeValue, DynamoDb, DynamoDbClient, GetItemInput, PutItemError, PutItemInput,
    PutItemOutput, UpdateItemError, UpdateItemInput, UpdateItemOutput,
};
use rusoto_s3::{GetObjectError, GetObjectOutput, GetObjectRequest, S3Client, S3};

use serde_json::Value;

use futures::stream::TryStreamExt;
use std::{collections::HashMap, env::var};

pub async fn get_item_from_table(
    primary_key_name: &str,
    primary_key_value: &str,
    table_name: &str,
) -> Option<HashMap<String, AttributeValue>> {
    let client = get_dynamo_client();

    let mut pkey = HashMap::new();
    pkey.insert(
        primary_key_name.to_string(),
        AttributeValue {
            s: Some(primary_key_value.to_string()),
            ..Default::default()
        },
    );

    let get_item_response = client
        .get_item(GetItemInput {
            key: pkey,
            table_name: table_name.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    get_item_response.item
}

pub async fn put_item_in_table(
    item: PutItemInput,
) -> Result<PutItemOutput, RusotoError<PutItemError>> {
    let client = get_dynamo_client();
    client.put_item(item).await
}

pub async fn update_item_in_table(
    item: UpdateItemInput,
) -> Result<UpdateItemOutput, RusotoError<UpdateItemError>> {
    let client = get_dynamo_client();
    client.update_item(item).await
}

pub fn get_dynamo_client() -> DynamoDbClient {
    ////! FIX: setup with onceCell global state
    DynamoDbClient::new(Region::default())
}

/// Combine two serde Value objects
/// # Example
///```
/// use serde_json::json;
/// use socless::json_merge;
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

pub fn get_s3_client() -> S3Client {
    ////! FIX: setup with onceCell global state
    S3Client::new(Region::default())
}

pub async fn get_object_from_s3(
    key: &str,
    bucket_name: &str,
) -> Result<GetObjectOutput, RusotoError<GetObjectError>> {
    let client = get_s3_client();
    let input = GetObjectRequest {
        bucket: bucket_name.to_owned(),
        key: key.to_owned(),
        ..Default::default()
    };
    client.get_object(input).await
}

pub async fn fetch_utf8_from_vault(key: &str) -> String {
    let socless_vault_bucket_name: String =
        var(&"SOCLESS_VAULT").expect("No env var found for SOCLESS_VAULT s3 bucket");

    let object_result = get_object_from_s3(key, &socless_vault_bucket_name).await;
    let object = object_result.expect(&format!("No object found for key: {}", key));

    let body = object.body.expect(&format!("No body in object: {}", key));

    let body = body
        .map_ok(|b| b.to_vec())
        .try_concat()
        .await
        .expect("Unable to convert ByteStream after S3 get_object");

    String::from_utf8(body).expect("S3 file is not valid utf8")
}
