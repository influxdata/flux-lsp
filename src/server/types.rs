use lspower::jsonrpc::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

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

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMeasurementParams {}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMeasurementResult {}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectTagParams {}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectTagResult {}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectTagValueParams {}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectTagValueResult {}

#[lspower::async_trait]
pub trait FluxLanguageServer: Send + Sync + 'static {
    async fn inject_measurement(
        &self,
        params: InjectMeasurementParams,
    ) -> lspower::jsonrpc::Result<InjectMeasurementResult>;
    async fn inject_tag(
        &self,
        params: InjectTagParams,
    ) -> lspower::jsonrpc::Result<InjectTagResult>;
    async fn inject_tag_value(
        &self,
        params: InjectTagValueParams,
    ) -> lspower::jsonrpc::Result<InjectTagValueResult>;
}
