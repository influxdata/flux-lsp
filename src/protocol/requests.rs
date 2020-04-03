use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::protocol::properties::{
    ContentChange, Position, TextDocument, TextDocumentIdentifier,
    VersionedTextDocumentIdentifier,
};

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
pub struct ReferenceContext {}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReferenceParams {
    pub context: ReferenceContext,
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
    pub position: Position,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InitializeParams {}

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
pub struct TextDocumentParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TextDocumentSaveParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TextDocumentChangeParams {
    #[serde(rename = "textDocument")]
    pub text_document: VersionedTextDocumentIdentifier,
    #[serde(rename = "contentChanges")]
    pub content_changes: Vec<ContentChange>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FoldingRangeParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocument,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DocumentSymbolParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompletionParams {
    pub context: Option<CompletionContext>,
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompletionContext {
    #[serde(rename = "triggerKind")]
    pub trigger_kind: i32,
    #[serde(rename = "triggerCharacter")]
    pub trigger_character: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SignatureHelpParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub context: Option<SignatureHelpContext>,
}

#[derive(Serialize_repr, Deserialize_repr, Clone)]
#[repr(u32)]
pub enum SignatureHelpTriggerKind {
    Invoked = 1,
    TriggerCharacter = 2,
    ContentChange = 3,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SignatureHelpContext {
    #[serde(rename = "isRetrigger")]
    pub is_retrigger: bool,
    #[serde(rename = "triggerCharacter")]
    pub trigger_character: Option<String>,
    #[serde(rename = "triggerKind")]
    pub trigger_kind: SignatureHelpTriggerKind,
}

/**
 * Value-object describing what options formatting should use.
 */
#[derive(Serialize, Deserialize, Clone)]
pub struct FormattingOptions {
    /**
     * Size of a tab in spaces.
     */
    #[serde(rename = "tabSize")]
    pub tab_size: u32,

    /**
     * Prefer spaces over tabs.
     */
    #[serde(rename = "insertSpaces")]
    pub insert_spaces: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DocumentFormattingParams {
    /**
     * The document to format.
     */
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,

    /**
     * The format options.
     */
    pub options: FormattingOptions,
}
