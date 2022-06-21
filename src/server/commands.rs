use serde::{Deserialize, Serialize};
use lspower::lsp;

// XXX: rockstar (15 Jun 2022) - Clippy will whinge here about every
// variant of this enum starts with "Inject". I'm not a fan of using
// the verb "inject" anyway, but this enum will eventually have many
// different commands that aren't at all about injection; we just happen
// to have hit the tipping point of enum size for this clippy lint to
// kick in. We can remove this `allow` when we add something that doesn't
// start with "Inject".
#[allow(clippy::enum_variant_names)]
pub enum LspServerCommand {
    InjectTagFilter,
    InjectTagValueFilter,
    InjectFieldFilter,
    InjectMeasurementFilter,
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
                Ok(LspServerCommand::InjectTagValueFilter)
            }
            "injectMeasurementFilter" => {
                Ok(LspServerCommand::InjectMeasurementFilter)
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
