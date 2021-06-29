use lspower::jsonrpc::Result;
use lspower::lsp::{
    InitializeParams, InitializeResult, ServerCapabilities,
    ServerInfo,
};
use lspower::LanguageServer;

#[allow(dead_code)]
struct LspServerOptions {
    folding: bool,
    influxdb_url: Option<String>,
    token: Option<String>,
    org: Option<String>,
}

#[allow(dead_code)]
pub struct LspServer {
    options: LspServerOptions,
}

impl LspServer {
    pub fn new(
        folding: bool,
        influxdb_url: Option<String>,
        token: Option<String>,
        org: Option<String>,
    ) -> Self {
        Self {
            options: LspServerOptions {
                folding,
                influxdb_url,
                token,
                org,
            },
        }
    }
}

#[lspower::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                call_hierarchy_provider: None,
                code_action_provider: None,
                code_lens_provider: None,
                color_provider: None,
                completion_provider: None,
                declaration_provider: None,
                definition_provider: None,
                document_formatting_provider: None,
                document_highlight_provider: None,
                document_link_provider: None,
                document_on_type_formatting_provider: None,
                document_range_formatting_provider: None,
                document_symbol_provider: None,
                execute_command_provider: None,
                experimental: None,
                folding_range_provider: None,
                hover_provider: None,
                implementation_provider: None,
                linked_editing_range_provider: None,
                moniker_provider: None,
                references_provider: None,
                rename_provider: None,
                selection_range_provider: None,
                semantic_tokens_provider: None,
                signature_help_provider: None,
                text_document_sync: None,
                type_definition_provider: None,
                workspace: None,
                workspace_symbol_provider: None,
            },
            server_info: Some(ServerInfo {
                name: "flux-lsp".to_string(),
                version: Some("2.0".to_string()),
            }),
        })
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use lspower::lsp::{ClientCapabilities, InitializeParams};
    use lspower::LanguageServer;
    use tokio_test::block_on;

    use super::LspServer;

    fn create_server() -> LspServer {
        LspServer::new(true, None, None, None)
    }

    #[test]
    fn test_initialized() {
        let server = create_server();

        let params = InitializeParams {
            capabilities: ClientCapabilities {
                workspace: None,
                text_document: None,
                window: None,
                general: None,
                experimental: None,
            },
            client_info: None,
            initialization_options: None,
            locale: None,
            process_id: None,
            root_path: None,
            root_uri: None,
            trace: None,
            workspace_folders: None,
        };

        let result = block_on(server.initialize(params)).unwrap();
        let server_info = result.server_info.unwrap();

        assert_eq!(server_info.name, "flux-lsp".to_string());
        assert_eq!(server_info.version, Some("2.0".to_string()));
    }

    #[test]
    fn test_shutdown() {
        let server = create_server();

        let result = block_on(server.shutdown()).unwrap();

        assert_eq!((), result)
    }
}
