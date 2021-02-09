//! SOCless core library, for Rust!
//! Allows users to write SOCless integrations (lambda functions) in Rust instead of Python.
//! Rust lambda functions have a very short init time, while Python can suffer slow 'cold starts' which can cause time-sensitive operations to fail if cold-starts are not mitigated with cloudwatch schedules or reserved concurrency.

//! What is SOCless?
//! [SOCless](https://twilio-labs.github.io/socless/) is an Automation Framework that provides a global state wrapper and helper utilities around AWS Step Functions.
//! SOCless allows users to write complex State Machines that can do more than pass a Step's output directly to the next step.
pub mod events;
pub mod helpers;
pub mod integrations;
pub mod utils;

pub use helpers::{
    fetch_utf8_from_vault, get_dynamo_client, get_object_from_s3, get_s3_client, json_merge,
    put_item_in_table, update_item_in_table,
};
pub use integrations::{socless_bootstrap, SoclessContext, SoclessLambdaEvent, StateConfig};
pub use utils::{gen_datetimenow, gen_id};

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
}
