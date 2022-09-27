use lspower::lsp;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(EnumIter)]
pub enum LspServerCommand {
    CompositionInitialize,
    AddMeasurementFilter,
    AddFieldFilter,
    RemoveFieldFilter,
    AddTagFilter,
    RemoveTagFilter,
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
            "fluxComposition/addMeasurementFilter" => {
                Ok(LspServerCommand::AddMeasurementFilter)
            }
            "fluxComposition/addFieldFilter" => {
                Ok(LspServerCommand::AddFieldFilter)
            }
            "fluxComposition/addTagFilter" => {
                Ok(LspServerCommand::AddTagFilter)
            }
            "fluxComposition/addTagValueFilter" => {
                Ok(LspServerCommand::AddTagValueFilter)
            }
            "fluxComposition/removeFieldFilter" => {
                Ok(LspServerCommand::RemoveFieldFilter)
            }
            "fluxComposition/removeTagFilter" => {
                Ok(LspServerCommand::RemoveTagFilter)
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
            LspServerCommand::AddMeasurementFilter => {
                "fluxComposition/addMeasurementFilter".into()
            }
            LspServerCommand::AddFieldFilter => {
                "fluxComposition/addFieldFilter".into()
            }
            LspServerCommand::AddTagFilter => {
                "fluxComposition/addTagFilter".into()
            }
            LspServerCommand::AddTagValueFilter => {
                "fluxComposition/addTagValueFilter".into()
            }
            LspServerCommand::RemoveFieldFilter => {
                "fluxComposition/removeFieldFilter".into()
            }
            LspServerCommand::RemoveTagFilter => {
                "fluxComposition/removeTagFilter".into()
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

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionInitializeParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub bucket: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
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
