use lspower::jsonrpc::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum LspError {
    InternalError(String),
    LockNotAcquired,
    FileNotFound(String),
    InvalidArguments,
}

impl From<LspError> for Error {
    fn from(error: LspError) -> Self {
        match error {
            LspError::InternalError(error) => Error {
                code: ErrorCode::InternalError,
                message: error,
                data: None,
            },
            LspError::LockNotAcquired => Error {
                code: ErrorCode::InternalError,
                message: "Could not acquire lock".into(),
                data: None,
            },
            LspError::FileNotFound(filename) => Error {
                code: ErrorCode::InvalidParams,
                message: format!("File not fiend: {}", filename),
                data: None,
            },
            LspError::InvalidArguments => Error {
                code: ErrorCode::InvalidParams,
                message: format!("Invalid arguments specified."),
                data: None,
            },
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMeasurementParams {}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectTagParams {}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectTagValueParams {}
