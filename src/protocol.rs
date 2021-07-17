use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use lspower::lsp;

fn default_id() -> u32 {
    0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseRequest {
    #[serde(default = "default_id")]
    pub id: u32,
    pub method: String,
}

impl BaseRequest {
    pub fn from_json(s: &str) -> Result<BaseRequest, String> {
        match serde_json::from_str(s) {
            Ok(c) => Ok(c),
            Err(_) => {
                Err("Failed to parse json of BaseRequest".to_string())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PolymorphicRequest {
    pub base_request: BaseRequest,
    pub data: String,
}

impl PolymorphicRequest {
    pub fn method(&self) -> String {
        self.base_request.method.clone()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Request<T> {
    #[serde(default = "default_id")]
    pub id: u32,
    pub method: String,
    pub params: Option<T>,
}

impl<T> Request<T>
where
    T: DeserializeOwned + Clone,
{
    pub fn from_json(s: &str) -> Result<Request<T>, String> {
        match serde_json::from_str(s) {
            Ok(c) => Ok(c),
            Err(e) => {
                Err(format!("Failed to parse json of Request: {}", e))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShutdownParams {}

#[derive(Serialize, Deserialize, Clone)]
pub struct Response<T> {
    pub id: u32,
    pub result: Option<T>,
    pub jsonrpc: String,
}

impl<T> Response<T>
where
    T: Serialize + Clone,
{
    pub fn new(id: u32, result: Option<T>) -> Response<T> {
        Response {
            id,
            result,
            jsonrpc: String::from("2.0"),
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        match serde_json::to_string(self) {
            Ok(s) => Ok(s),
            Err(_) => {
                Err("Failed to serialize initialize response"
                    .to_string())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShutdownResult {}

#[derive(Serialize, Deserialize)]
pub struct ShowMessageParams {
    #[serde(rename = "type")]
    pub message_type: u32,
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct PublishDiagnosticsParams {
    pub uri: lsp::Url,
    pub diagnostics: Vec<lsp::Diagnostic>,
}

#[derive(Serialize, Deserialize)]
pub struct Notification<T> {
    method: String,
    params: T,
}

impl<T> Notification<T>
where
    T: Serialize,
{
    pub fn to_json(&self) -> Result<String, String> {
        match serde_json::to_string(self) {
            Ok(s) => Ok(s),
            Err(_) => Err(String::from(
                "Failed to serialize initialize response",
            )),
        }
    }
}

pub fn create_diagnostics_notification(
    uri: lsp::Url,
    diagnostics: Vec<lsp::Diagnostic>,
) -> Notification<PublishDiagnosticsParams> {
    let method = String::from("textDocument/publishDiagnostics");
    let params = PublishDiagnosticsParams { uri, diagnostics };
    Notification { method, params }
}
