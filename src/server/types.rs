#[derive(Debug)]
pub enum LspError {
    InternalError(String),
    LockNotAcquired,
    FileNotFound(String),
}

impl Into<lspower::jsonrpc::Error> for LspError {
    fn into(self) -> lspower::jsonrpc::Error {
        match self {
            LspError::InternalError(error) => {
                lspower::jsonrpc::Error {
                    code: lspower::jsonrpc::ErrorCode::InternalError,
                    message: error,
                    data: None,
                }
            }
            LspError::LockNotAcquired => lspower::jsonrpc::Error {
                code: lspower::jsonrpc::ErrorCode::InternalError,
                message: "Could not acquire lock".into(),
                data: None,
            },
            LspError::FileNotFound(filename) => {
                lspower::jsonrpc::Error {
                    code: lspower::jsonrpc::ErrorCode::InvalidParams,
                    message: format!("File not fiend: {}", filename),
                    data: None,
                }
            },
        }
    }
}
