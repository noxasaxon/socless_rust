use chrono::Utc;
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
