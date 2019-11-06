use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_id() -> u32 {
    0
}

fn default_version() -> u32 {
    1
}

fn default_language_id() -> String {
    "".to_string()
}

fn default_text() -> String {
    "".to_string()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub message: String,
    pub severity: u32,
    pub code: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Location {
    pub uri: String,
    pub range: Range,
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

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Clone)]
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
pub struct ShutdownParams {}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReferenceContext {}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReferenceParams {
    pub context: ReferenceContext,
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
    pub position: Position,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InitializeRequestParams {}

#[derive(Serialize, Deserialize, Clone)]
pub struct RenameParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
    pub position: Position,
    #[serde(rename = "newName")]
    pub new_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TextDocumentPositionParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
    pub position: Position,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TextDocument {
    pub uri: String,
    #[serde(rename = "languageId", default = "default_language_id")]
    pub language_id: String,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_text")]
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TextDocumentParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ServerCapabilities {
    #[serde(rename = "referencesProvider")]
    references_provider: bool,

    #[serde(rename = "definitionProvider")]
    definition_provider: bool,

    #[serde(rename = "renameProvider")]
    rename_provider: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShutdownResult {}

#[derive(Serialize, Deserialize, Clone)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
}

impl Default for InitializeResult {
    fn default() -> Self {
        InitializeResult {
            capabilities: ServerCapabilities {
                definition_provider: true,
                references_provider: true,
                rename_provider: true,
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize, Deserialize)]
pub struct ShowMessageParams {
    #[serde(rename = "type")]
    pub message_type: u32,
    message: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TextEdit {
    #[serde(rename = "newText")]
    pub new_text: String,
    pub range: Range,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceEditResult {
    pub changes: HashMap<String, Vec<TextEdit>>,
}

pub fn create_diagnostics_notification(
    uri: String,
    diagnostics: Vec<Diagnostic>,
) -> Result<Notification<PublishDiagnosticsParams>, &'static str> {
    let method = String::from("textDocument/publishDiagnostics");
    let params = PublishDiagnosticsParams { uri, diagnostics };
    let request = Notification { method, params };

    Ok(request)
}
