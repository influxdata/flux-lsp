use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request, Response};

use lspower::lsp;

pub struct InitializeHandler {
    disable_folding: bool,
}

impl InitializeHandler {
    pub fn new(disable_folding: bool) -> InitializeHandler {
        InitializeHandler { disable_folding }
    }
}

#[async_trait::async_trait]
impl RequestHandler for InitializeHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
        _: &Cache,
    ) -> Result<Option<String>, Error> {
        let _: Request<lsp::InitializeParams> =
            Request::from_json(prequest.data.as_str())?;
        let result = lsp::InitializeResult {
            capabilities: lsp::ServerCapabilities {
                call_hierarchy_provider: None,
                code_action_provider: None,
                code_lens_provider: None,
                color_provider: None,
                completion_provider: Some(lsp::CompletionOptions {
                    resolve_provider: Some(true),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        ":".to_string(),
                        "(".to_string(),
                        ",".to_string(),
                        "\"".to_string(),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options:
                        lsp::WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                }),
                declaration_provider: None,
                definition_provider: Some(lsp::OneOf::Left(true)),
                document_formatting_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                document_highlight_provider: None,
                document_link_provider: None,
                document_on_type_formatting_provider: None,
                document_range_formatting_provider: None,
                document_symbol_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                execute_command_provider: None,
                experimental: None,
                folding_range_provider: Some(
                    lsp::FoldingRangeProviderCapability::Simple(
                        !self.disable_folding,
                    ),
                ),
                hover_provider: Some(
                    lsp::HoverProviderCapability::Simple(true),
                ),
                implementation_provider: None,
                linked_editing_range_provider: None,
                moniker_provider: None,
                references_provider: Some(lsp::OneOf::Left(true)),
                rename_provider: Some(lsp::OneOf::Left(true)),
                selection_range_provider: None,
                semantic_tokens_provider: None,
                signature_help_provider: Some(
                    lsp::SignatureHelpOptions {
                        trigger_characters: Some(vec![
                            "(".to_string()
                        ]),
                        retrigger_characters: Some(vec![
                            "(".to_string()
                        ]),
                        work_done_progress_options:
                            lsp::WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                    },
                ),
                text_document_sync: Some(
                    lsp::TextDocumentSyncCapability::Kind(
                        lsp::TextDocumentSyncKind::Full,
                    ),
                ),
                type_definition_provider: None,
                workspace: None,
                workspace_symbol_provider: None,
            },
            server_info: Some(lsp::ServerInfo {
                name: "flux-lsp".to_string(),
                version: Some("1.0".to_string()),
            }),
        };
        let response =
            Response::new(prequest.base_request.id, Some(result));

        let json = response.to_json()?;

        Ok(Some(json))
    }
}
