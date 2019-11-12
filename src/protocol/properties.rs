use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ServerCapabilities {
    #[serde(rename = "referencesProvider")]
    pub references_provider: bool,

    #[serde(rename = "definitionProvider")]
    pub definition_provider: bool,

    #[serde(rename = "renameProvider")]
    pub rename_provider: bool,

    #[serde(rename = "foldingRangeProvider")]
    pub folding_range_provider: bool,
}
