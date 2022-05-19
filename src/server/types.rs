use lspower::jsonrpc::{Error, ErrorCode};

#[derive(Debug)]
pub enum LspError {
    InternalError(String),
    LockNotAcquired,
    FileNotFound(String),
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
        }
    }
}
