use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;

use crate::protocol::properties::TextEdit;

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
pub struct WorkspaceEditResult {
    pub changes: HashMap<String, Vec<TextEdit>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CompletionList {
    #[serde(rename = "isIncomplete")]
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}
#[derive(Serialize_repr, Deserialize_repr, Clone, Debug)]
#[repr(u32)]
pub enum InsertTextFormat {
    PlainText = 1,
    Snippet = 2,
}

#[derive(Serialize_repr, Deserialize_repr, Clone, Debug)]
#[repr(u32)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<CompletionItemKind>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub deprecated: bool,
    pub preselect: Option<bool>,
    #[serde(rename = "sortText")]
    pub sort_text: Option<String>,
    #[serde(rename = "filterText")]
    pub filter_text: Option<String>,
    #[serde(rename = "insertText")]
    pub insert_text: Option<String>,
    #[serde(rename = "commitCharacters")]
    pub commit_characters: Option<Vec<String>>,
    #[serde(rename = "insertTextFormat")]
    pub insert_text_format: InsertTextFormat,
    #[serde(rename = "textEdit")]
    pub text_edit: Option<TextEdit>,
    #[serde(rename = "additionalTextEdits")]
    pub additional_text_edits: Option<Vec<TextEdit>>,
}

impl CompletionItem {
    pub fn new(name: String, package: String) -> Self {
        CompletionItem {
            label: format!("{} ({})", name, package),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some(format!("package: {}", package)),
            documentation: Some(format!("package: {}", package)),
            filter_text: Some(name.clone()),
            insert_text: Some(name.clone()),
            insert_text_format: InsertTextFormat::PlainText,
            kind: Some(CompletionItemKind::Function),
            preselect: None,
            sort_text: Some(format!("{} {}", name, package)),
            text_edit: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HoverResult {
    pub contents: MarkupContent,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MarkupContent {
    pub kind: String,
    pub value: String,
}

impl MarkupContent {
    pub fn new(content: String) -> Self {
        MarkupContent {
            kind: "markdown".to_string(),
            value: content,
        }
    }
}
