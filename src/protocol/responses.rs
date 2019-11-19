use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::protocol::properties::{
    ServerCapabilities, TextDocumentSyncKind, TextEdit,
};

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

#[derive(Serialize, Deserialize, Clone)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
}

impl InitializeResult {
    pub fn new(folding_range_provider: bool) -> Self {
        InitializeResult {
            capabilities: ServerCapabilities {
                definition_provider: true,
                references_provider: true,
                rename_provider: true,
                folding_range_provider,
                document_symbol_provider: true,
                text_document_sync: TextDocumentSyncKind::Full,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceEditResult {
    pub changes: HashMap<String, Vec<TextEdit>>,
}
