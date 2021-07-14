use serde::{Deserialize, Serialize};

use lspower::lsp;

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
