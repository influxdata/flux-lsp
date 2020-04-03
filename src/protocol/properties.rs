use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use flux::ast::SourceLocation;

#[derive(Serialize_repr, Deserialize_repr, Clone)]
#[repr(u32)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

pub fn loc_to_range(loc: &SourceLocation) -> Range {
    Range {
        start: Position {
            character: loc.start.column - 1,
            line: loc.start.line - 1,
        },
        end: Position {
            character: loc.end.column - 1,
            line: loc.end.line - 1,
        },
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub deprecated: Option<bool>,
    pub range: Range,
    #[serde(rename = "selectionRange")]
    pub selection_range: Range,
    pub children: Option<Vec<DocumentSymbol>>,
}

impl DocumentSymbol {
    pub fn new(
        kind: SymbolKind,
        name: String,
        detail: String,
        loc: &SourceLocation,
    ) -> DocumentSymbol {
        let range = loc_to_range(loc);
        DocumentSymbol {
            children: None,
            deprecated: None,
            kind,
            selection_range: range.clone(),
            range,
            name,
            detail: Some(detail),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SymbolInformation {
    pub name: String,
    pub kind: SymbolKind,
    pub deprecated: Option<bool>,
    pub location: Location,

    #[serde(rename = "containerName")]
    pub container_name: Option<String>,
}

impl SymbolInformation {
    pub fn new(
        kind: SymbolKind,
        name: String,
        uri: String,
        loc: &SourceLocation,
    ) -> SymbolInformation {
        SymbolInformation {
            name,
            kind,
            deprecated: Some(false),
            container_name: None,
            location: Location {
                uri,
                range: loc_to_range(loc),
            },
        }
    }
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

#[derive(Serialize, Deserialize, Clone, Eq, Debug, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn move_back(&self, count: u32) -> Self {
        Position {
            line: self.line,
            character: self.character - count,
        }
    }

    pub fn new(line: u32, character: u32) -> Position {
        Position { line, character }
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.character == other.character && self.line == other.line
    }
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextEdit {
    #[serde(rename = "newText")]
    pub new_text: String,
    pub range: Range,
}

impl PartialEq for TextEdit {
    fn eq(&self, other: &Self) -> bool {
        self.new_text == other.new_text && self.range == other.range
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, Debug)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl PartialEq for Range {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end == other.end
    }
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

#[derive(Serialize, Clone)]
pub struct CompletionOptions {
    #[serde(rename = "resolveProvider")]
    pub resolve_provider: Option<bool>,
    #[serde(rename = "triggerCharacters")]
    pub trigger_characters: Option<Vec<String>>,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: Some(vec![
                ".".to_string(),
                ":".to_string(),
                "(".to_string(),
                ",".to_string(),
            ]),
        }
    }
}

#[derive(Serialize, Clone)]
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

    #[serde(rename = "documentSymbolProvider")]
    pub document_symbol_provider: bool,

    #[serde(rename = "completionProvider")]
    pub completion_provider: CompletionOptions,

    #[serde(rename = "signatureHelpProvider")]
    pub signature_help_provider: SignatureHelpOptions,
    #[serde(rename = "documentFormattingProvider")]
    pub document_formatting_provider: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContentChange {
    pub text: String,
    pub range: Option<Range>,
    #[serde(rename = "rangeLength")]
    pub range_length: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SignatureHelpOptions {
    #[serde(rename = "triggerCharacters")]
    pub trigger_characters: Option<Vec<String>>,
    #[serde(rename = "retriggerCharacters")]
    pub retrigger_characters: Option<Vec<String>>,
}

impl Default for SignatureHelpOptions {
    fn default() -> Self {
        SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string()]),
            retrigger_characters: Some(vec!["(".to_string()]),
        }
    }
}
