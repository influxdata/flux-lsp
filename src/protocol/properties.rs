use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

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
                "\"".to_string(), // NOTE: trigger at the beginning of a string argument
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

    #[serde(rename = "documentFormattingProvider")]
    pub document_formatting_provider: bool,

    #[serde(rename = "completionProvider")]
    pub completion_provider: CompletionOptions,

    #[serde(rename = "signatureHelpProvider")]
    pub signature_help_provider: SignatureHelpOptions,

    #[serde(rename = "hoverProvider")]
    pub hover_provider: bool,
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
