use thiserror::Error;
// https://github.com/dtolnay/thiserror

// #[derive(Error, Debug)]
// pub enum SoclessError {
//     #[error("key: {} not found in table: {}")]
//     NotFoundError { key: String, table: String },
//     // NotFoundError(#[from] io::Error),
//     #[error("the data for key `{0}` is not available")]
//     Redaction(String),
//     #[error("invalid header (expected {expected:?}, found {found:?})")]
//     InvalidHeader { expected: String, found: String },
//     #[error("unknown data store error")]
//     Unknown,
// }

#[derive(Error, Debug)]
pub enum SoclessError {
    #[error("key: {key} not found in table: {found}")]
    NotFoundError { key: String, table: String },
    // NotFoundError(#[from] io::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader { expected: String, found: String },
    // #[error("unknown data store error")]
    // Unknown,
}
