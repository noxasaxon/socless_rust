use aws_sdk_dynamodb::{
    input::PutItemInput,
    model::{AttributeValue, DeleteRequest, KeysAndAttributes, PutRequest, WriteRequest},
};
use serde_dynamo::aws_sdk_dynamodb_0_4::{from_item, from_items, to_attribute_value, to_item};
use serde_json::Value;
use std::{collections::HashMap, env::var};

use crate::clients::get_or_init_dynamo;

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
        .expect(&format!(
            "Error in get_item of table: {} for key= {{ {} : {} }}",
            table_name, primary_key_name, primary_key_value
        ));

    result.item

    // let mut pkey = HashMap::new();
    // pkey.insert(
    //     primary_key_name.to_string(),
    //     AttributeValue {
    //         s: Some(primary_key_value.to_string()),
    //         ..Default::default()
    //     },
    // );
}

///
///
/// # Example
/// ```ignore
/// put_item_in_table(PutItemInput {
///     item: to_item(event_table_input.clone()).unwrap(),
///     table_name: events_table_name.to_owned(),
///     ..Default::default()
/// })
/// .await
/// .unwrap();
/// ```
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

/// Search string for a given pattern, return a Tuple of (before_pattern, pattern, after_pattern)
/// # Example
/// ```
/// use socless::split_with_delimiter;
/// let result = split_with_delimiter("something.something!json", "!");
/// assert_eq!(result, Some(("something.something".to_string(), "!".to_string(), "json".to_string())));
/// ```
pub fn split_with_delimiter(string: &str, delimiter: &str) -> Option<(String, String, String)> {
    let searched: Vec<&str> = string.splitn(2, delimiter).collect();
    return if searched.len() <= 1 {
        None
    } else {
        Some((
            searched[0].to_string(),
            delimiter.to_string(),
            searched[1].to_string(),
        ))
    };
}

#[cfg(test)]
mod tests {
    use crate::split_with_delimiter;

    #[test]
    fn test_split_with_delimiter() {
        let result = split_with_delimiter("something.something!json", "!");
        assert_eq!(
            result,
            Some((
                "something.something".to_string(),
                "!".to_string(),
                "json".to_string()
            ))
        );
    }

    #[test]
    fn test_split_with_delimiter_not_found() {
        let result = split_with_delimiter("something.somethingjson", "!");
        assert_eq!(result, None);
    }

    #[test]
    fn test_split_with_delimiter_appear_twice() {
        let result = split_with_delimiter("something.something!!json", "!");
        assert_eq!(
            result,
            Some((
                "something.something".to_string(),
                "!".to_string(),
                "!json".to_string()
            ))
        );
    }

    #[test]
    fn test_split_with_delimiter_appear_at_end() {
        let result = split_with_delimiter("something.somethingjson!", "!");
        assert_eq!(
            result,
            Some((
                "something.somethingjson".to_string(),
                "!".to_string(),
                "".to_string()
            ))
        );
    }

    #[test]
    fn test_split_with_delimiter_appear_at_beginning() {
        let result = split_with_delimiter("!something.somethingjson", "!");
        assert_eq!(
            result,
            Some((
                "".to_string(),
                "!".to_string(),
                "something.somethingjson".to_string()
            ))
        );
    }
}
