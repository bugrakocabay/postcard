//! curl command import (`parse_curl`) and export (`to_curl`).

mod export;
mod parse;
mod tokenize;

pub use export::to_curl;
pub use parse::{ParsedCurl, parse_curl};
pub use tokenize::tokenize;

/// Why a curl command could not be imported.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum CurlError {
    #[error("not a curl command")]
    NotCurl,
    #[error("unterminated quote")]
    UnterminatedQuote,
    #[error("no URL found in the curl command")]
    MissingUrl,
    #[error("missing argument for {0}")]
    MissingArgument(String),
}
