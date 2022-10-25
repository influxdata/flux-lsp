use lspower::jsonrpc::{Error, ErrorCode};

#[derive(Debug)]
pub enum LspError {
    InternalError(String),
    LockNotAcquired,
    FileNotFound(String),
    InvalidArguments(Vec<serde_json::value::Value>),
    InvalidCommand(String),

    CompositionNotFound(lspower::lsp::Url),
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
            LspError::InvalidArguments(value) => Error {
                code: ErrorCode::InvalidParams,
                message: format!(
                    "Invalid parameters supplied: {:?}",
                    value
                ),
                data: None,
            },
            LspError::InvalidCommand(command) => Error {
                code: ErrorCode::InvalidParams,
                message: format!(
                    "Unknown command execution: {}",
                    command
                ),
                data: None,
            },

            LspError::CompositionNotFound(uri) => Error {
                code: ErrorCode::InvalidParams,
                message: format!(
                    "Composition not found for uri: {}",
                    uri
                ),
                data: None,
            },
        }
    }
}
