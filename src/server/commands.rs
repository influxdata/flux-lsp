use lspower::lsp;
use serde::{Deserialize, Serialize};

pub enum LspServerCommand {
    InjectTagFilter,
    InjectTagValueFilter,
    InjectFieldFilter,
    InjectMeasurementFilter,
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
            "injectTagFilter" => {
                Ok(LspServerCommand::InjectTagFilter)
            }
            "injectTagValueFilter" => {
                Ok(LspServerCommand::InjectTagValueFilter)
            }
            "injectFieldFilter" => {
                Ok(LspServerCommand::InjectFieldFilter)
            }
            "injectMeasurementFilter" => {
                Ok(LspServerCommand::InjectMeasurementFilter)
            }
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
            LspServerCommand::InjectTagFilter => {
                "injectTagFilter".into()
            }
            LspServerCommand::InjectTagValueFilter => {
                "injectTagValueFilter".into()
            }
            LspServerCommand::InjectFieldFilter => {
                "injectFieldFilter".into()
            }
            LspServerCommand::InjectMeasurementFilter => {
                "injectMeasurementFilter".into()
            }
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
pub struct InjectTagFilterParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub bucket: String,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectTagValueFilterParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub bucket: String,
    pub name: String,
    pub value: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectFieldFilterParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub bucket: String,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMeasurementFilterParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub bucket: String,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionInitializeParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub bucket: String,
    pub measurement: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueFilterParams {
    pub text_document: lsp::TextDocumentIdentifier,
    pub name: String,
    pub value: String,
}
