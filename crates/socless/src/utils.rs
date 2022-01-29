use crate::clients::get_or_init_dynamo;
use aws_sdk_dynamodb::model::AttributeValue;
use chrono::Utc;
use serde_dynamo::to_attribute_value;
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
