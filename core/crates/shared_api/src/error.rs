use std::fmt;

/// A small, caller-agnostic error type for helpers shared between the HTTP
/// server and the FFI bridge. Callers map `BadRequest`/`Internal` to their own
/// error representation.
#[derive(Debug)]
pub enum SharedApiError {
    BadRequest(String),
    Internal(String),
}

impl fmt::Display for SharedApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(message) | Self::Internal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for SharedApiError {}

impl From<std::io::Error> for SharedApiError {
    fn from(value: std::io::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<url::ParseError> for SharedApiError {
    fn from(value: url::ParseError) -> Self {
        Self::BadRequest(value.to_string())
    }
}

impl From<reqwest::Error> for SharedApiError {
    fn from(value: reqwest::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<std::string::FromUtf8Error> for SharedApiError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<storage::StorageError> for SharedApiError {
    fn from(value: storage::StorageError) -> Self {
        Self::Internal(value.to_string())
    }
}
