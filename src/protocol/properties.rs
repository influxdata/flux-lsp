use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

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
pub struct TextDocumentIdentifier {
    pub uri: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VersionedTextDocumentIdentifier {
    pub uri: String,
    #[serde(default = "default_version")]
    pub version: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TextEdit {
    #[serde(rename = "newText")]
    pub new_text: String,
    pub range: Range,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FoldingRange {
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "startCharacter")]
    pub start_character: u32,
    #[serde(rename = "endLine")]
    pub end_line: u32,
    #[serde(rename = "endCharacter")]
    pub end_character: u32,
    pub kind: String,
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

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Clone)]
#[repr(u8)]
pub enum TextDocumentSyncKind {
    /**
     * Documents should not be synced at all.
     */
    None = 0,
    /**
     * Documents are synced by always sending the full content of the document.
     */
    Full = 1,
    /**
     * Documents are synced by sending the full content on open. After that only incremental
     * updates to the document are sent.
     */
    Incremental = 2,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerCapabilities {
    #[serde(rename = "textDocumentSync")]
    pub text_document_sync: TextDocumentSyncKind,

    #[serde(rename = "referencesProvider")]
    pub references_provider: bool,

    #[serde(rename = "definitionProvider")]
    pub definition_provider: bool,

    #[serde(rename = "renameProvider")]
    pub rename_provider: bool,

    #[serde(rename = "foldingRangeProvider")]
    pub folding_range_provider: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContentChange {
    pub text: String,
    pub range: Option<Range>,
    #[serde(rename = "rangeLength")]
    pub range_length: Option<u32>,
}
