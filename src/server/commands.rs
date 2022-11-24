use lspower::lsp;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

#[derive(EnumIter)]
pub enum LspServerCommand {
    CompositionInitialize,
    SetMeasurementFilter,
    AddFieldFilter,
    RemoveFieldFilter,
    AddTagValueFilter,
    RemoveTagValueFilter,
    GetFunctionList,
}

impl TryFrom<String> for LspServerCommand {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "fluxComposition/initialize" => {
                Ok(LspServerCommand::CompositionInitialize)
            }
            "fluxComposition/setMeasurementFilter" => {
                Ok(LspServerCommand::SetMeasurementFilter)
            }
            "fluxComposition/addFieldFilter" => {
                Ok(LspServerCommand::AddFieldFilter)
            }
            "fluxComposition/addTagValueFilter" => {
                Ok(LspServerCommand::AddTagValueFilter)
            }
            "fluxComposition/removeFieldFilter" => {
                Ok(LspServerCommand::RemoveFieldFilter)
            }
            "fluxComposition/removeTagValueFilter" => {
                Ok(LspServerCommand::RemoveTagValueFilter)
            }
            "getFunctionList" => {
                Ok(LspServerCommand::GetFunctionList)
            }
            _ => Err(format!(
                "Received unknown value for LspServerCommand: {}",
                value
            )),
        }
    }
}

impl From<LspServerCommand> for String {
    fn from(value: LspServerCommand) -> Self {
        match value {
            LspServerCommand::CompositionInitialize => {
                "fluxComposition/initialize".into()
            }
            LspServerCommand::SetMeasurementFilter => {
                "fluxComposition/setMeasurementFilter".into()
            }
            LspServerCommand::AddFieldFilter => {
                "fluxComposition/addFieldFilter".into()
            }
            LspServerCommand::AddTagValueFilter => {
                "fluxComposition/addTagValueFilter".into()
            }
            LspServerCommand::RemoveFieldFilter => {
                "fluxComposition/removeFieldFilter".into()
            }
            LspServerCommand::RemoveTagValueFilter => {
                "fluxComposition/removeTagValueFilter".into()
            }
            LspServerCommand::GetFunctionList => {
                "getFunctionList".into()
            }
        }
    }
}

#[derive(Debug)]
pub enum LspClientCommand {
    UpdateComposition,
    CompositionDropped,
    CompositionNotFound,
}

impl ToString for LspClientCommand {
    fn to_string(&self) -> String {
        match &self {
            LspClientCommand::UpdateComposition => {
                "fluxComposition/compositionState".into()
            }
            LspClientCommand::CompositionDropped => {
                "fluxComposition/compositionEnded".into()
            }
            LspClientCommand::CompositionNotFound => {
                "fluxComposition/compositionSyncFailed".into()
            }
        }
    }
}

#[derive(Debug, Display)]
pub enum LspMessageActionItem {
    CompositionRange,
    CompositionState,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionInitializeParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub bucket: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
    #[deprecated(
        since = "0.8.36",
        note = "tag filters are no longer supported"
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_values: Option<Vec<(String, String)>>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueFilterParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub value: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagValueFilterParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub tag: String,
    pub value: String,
}
