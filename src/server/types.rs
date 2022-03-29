#[derive(Debug)]
pub enum LspError {
    InternalError(String),
    LockNotAcquired,
    FileNotFound(String),
}

impl From<LspError> for tower_lsp::jsonrpc::Error {
    fn from(error: LspError) -> Self {
        match error {
            LspError::InternalError(error) => {
                tower_lsp::jsonrpc::Error {
                    code:
                        tower_lsp::jsonrpc::ErrorCode::InternalError,
                    message: error,
                    data: None,
                }
            }
            LspError::LockNotAcquired => tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                message: "Could not acquire lock".into(),
                data: None,
            },
            LspError::FileNotFound(filename) => {
                tower_lsp::jsonrpc::Error {
                    code:
                        tower_lsp::jsonrpc::ErrorCode::InvalidParams,
                    message: format!("File not fiend: {}", filename),
                    data: None,
                }
            }
        }
    }
}
