use serde::{Deserialize, Serialize};

fn default_id() -> u32 {
    return 0;
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
            Ok(c) => return Ok(c),
            Err(_) => return Err("Failed to parse json of BaseRequest".to_string()),
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
        return self.base_request.method.clone();
    }
}

#[derive(Serialize, Deserialize)]
pub struct InitializeRequest {}

impl InitializeRequest {
    pub fn from_json(s: &str) -> Result<InitializeRequest, String> {
        match serde_json::from_str(s) {
            Ok(c) => return Ok(c),
            Err(_) => return Err("Failed to parse json of InitializeRequest".to_string()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TextDocument {
    pub uri: String,
    #[serde(rename = "languageId")]
    pub language_id: String,
    pub version: u32,
    pub text: String,
}

#[derive(Serialize, Deserialize)]
pub struct TextDocumentDidOpenParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
}

#[derive(Serialize, Deserialize)]
pub struct TextDocumentDidOpenRequest {
    #[serde(default = "default_id")]
    pub id: u32,
    pub method: String,
    pub params: TextDocumentDidOpenParams,
}

impl TextDocumentDidOpenRequest {
    pub fn from_json(s: &str) -> Result<TextDocumentDidOpenRequest, String> {
        match serde_json::from_str(s) {
            Ok(c) => return Ok(c),
            Err(e) => {
                return Err(format!(
                    "Failed to parse json of TextDocumentDidOpenRequest {}",
                    e
                ))
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(rename = "hoverProvider")]
    pub hover_provider: bool,
    #[serde(rename = "renameProvider")]
    pub rename_provider: bool,
}

#[derive(Serialize, Deserialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
}

impl InitializeResult {
    pub fn new() -> InitializeResult {
        return InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: true,
                rename_provider: true,
            },
        };
    }
}

#[derive(Serialize, Deserialize)]
pub struct InitializeResponse {
    pub id: u32,
    pub result: InitializeResult,
    pub jsonrpc: String,
}

impl InitializeResponse {
    pub fn new(id: u32, result: InitializeResult) -> InitializeResponse {
        return InitializeResponse {
            jsonrpc: String::from("2.0"),
            result: result,
            id: id,
        };
    }

    pub fn to_json(&self) -> Result<String, String> {
        match serde_json::to_string(self) {
            Ok(s) => return Ok(s),
            Err(_) => return Err(String::from("Failed to serialize initialize response")),
        };
    }
}
