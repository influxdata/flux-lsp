use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use log::{debug, error};
use lspower::jsonrpc::Result;
use lspower::lsp;
use lspower::LanguageServer;

use crate::handlers::find_node;
use crate::handlers::signature_help::find_stdlib_signatures;

fn parse_and_analyze(code: &str) -> flux::semantic::nodes::Package {
    let file = flux::parser::parse_string("", code);
    let ast_pkg = flux::ast::Package {
        base: file.base.clone(),
        path: "".to_string(),
        package: "main".to_string(),
        files: vec![file],
    };
    flux::semantic::convert::convert_with(
        ast_pkg,
        &mut flux::semantic::fresh::Fresher::default(),
    )
    .unwrap()
}

#[allow(dead_code)]
struct LspServerOptions {
    folding: bool,
    influxdb_url: Option<String>,
    token: Option<String>,
    org: Option<String>,
}

#[allow(dead_code)]
pub struct LspServer {
    store: Arc<Mutex<HashMap<lsp::Url, String>>>,
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
            store: Arc::new(Mutex::new(HashMap::new())),
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
        _: lsp::InitializeParams,
    ) -> Result<lsp::InitializeResult> {
        Ok(lsp::InitializeResult {
            capabilities: lsp::ServerCapabilities {
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
            server_info: Some(lsp::ServerInfo {
                name: "flux-lsp".to_string(),
                version: Some("2.0".to_string()),
            }),
        })
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    async fn did_open(
        &self,
        params: lsp::DidOpenTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;
        let value = params.text_document.text;
        self.store.lock().unwrap().insert(key, value);
    }
    async fn signature_help(
        &self,
        params: lsp::SignatureHelpParams,
    ) -> Result<Option<lsp::SignatureHelp>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let store = self.store.lock().unwrap();
        if !store.contains_key(&key) {
            // File isn't loaded into memory
            error!(
                "signature help failed: file {} not open on server",
                key
            );
            return Err(lspower::jsonrpc::Error::invalid_params(
                format!("file not opened: {}", key),
            ));
        }

        let mut signatures = vec![];
        let data = store.get(&key).unwrap();

        let pkg = parse_and_analyze(&data);
        let node_finder_result = find_node(
            flux::semantic::walk::Node::Package(&pkg),
            params.text_document_position_params.position,
        );

        if let Some(node) = node_finder_result.node {
            if let flux::semantic::walk::Node::CallExpr(call) =
                node.as_ref()
            {
                let callee = call.callee.clone();

                if let flux::semantic::nodes::Expression::Member(member) = callee.clone() {
                    let name = member.property.clone();
                    if let flux::semantic::nodes::Expression::Identifier(ident) = member.object.clone() {
                        signatures.extend(find_stdlib_signatures(name, ident.name));
                    }
                } else if let flux::semantic::nodes::Expression::Identifier(ident) = callee {
                    signatures.extend(find_stdlib_signatures(
                            ident.name,
                            "builtin".to_string()));
                    // XXX: rockstar (13 Jul 2021) - Add support for user defined
                    // signatures.
                } else {
                    debug!("signature_help on non-member and non-identifier");
                }
            } else {
                debug!("signature_help on non-call expression");
            }
        }

        // XXX: rockstar (12 Jul 2021) - `active_parameter` and `active_signature`
        // are currently unsupported, as they were unsupported in the previous
        // version of the server. They should be implemented, as it presents a
        // much better user interface.
        let response = lsp::SignatureHelp {
            signatures,
            active_signature: None,
            active_parameter: None,
        };
        Ok(Some(response))
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use lspower::lsp;
    use lspower::LanguageServer;
    use tokio_test::block_on;

    use super::LspServer;

    #[allow(dead_code)]
    const SIGNATURE_HELP: &'static str =
        include_str!("../tests/fixtures/signatures.flux");

    fn create_server() -> LspServer {
        LspServer::new(true, None, None, None)
    }

    fn open_file(server: &LspServer, text: String) {
        let params = lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem::new(
                lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                "flux".to_string(),
                1,
                text,
            ),
        };
        block_on(server.did_open(params));
    }

    #[test]
    fn test_initialized() {
        let server = create_server();

        let params = lsp::InitializeParams {
            capabilities: lsp::ClientCapabilities {
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

    #[test]
    fn test_did_open() {
        let server = create_server();
        let params = lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem::new(
                lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                "flux".to_string(),
                1,
                "from(".to_string(),
            ),
        };

        let result = block_on(server.did_open(params));

        assert_eq!((), result);
    }

    // If the file hasn't been opened on the server get, return an error.
    #[test]
    fn test_signature_help_not_opened() {
        let server = create_server();

        let params = lsp::SignatureHelpParams {
            context: None,
            text_document_position_params:
                lsp::TextDocumentPositionParams::new(
                    lsp::TextDocumentIdentifier::new(
                        lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    ),
                    lsp::Position::new(1, 1),
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let result = block_on(server.signature_help(params));

        assert!(result.is_err());
    }

    #[test]
    fn test_signature_help() {
        let server = create_server();
        open_file(&server, "from(".to_string());

        // XXX: rockstar (13 Jul 2021) - In the lsp protocol, Position arguments
        // are indexed from 1, e.g. there is no line number 0. This references
        // (0, 5) for compatibility with the previous implementation, but should
        // be updated to (1, 5) at some point.
        let params = lsp::SignatureHelpParams {
            context: None,
            text_document_position_params:
                lsp::TextDocumentPositionParams::new(
                    lsp::TextDocumentIdentifier::new(
                        lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    ),
                    lsp::Position::new(0, 5),
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let result =
            block_on(server.signature_help(params)).unwrap().unwrap();

        // The signatures returned from this test are...many. This test checks
        // the length of the signatures, and that a specific
        // `lsp::SignatureInformation` is contained within.
        let expected_signature_information =
            lsp::SignatureInformation {
                label: "from(bucket: $bucket)".to_string(),
                documentation: None,
                parameters: Some(vec![lsp::ParameterInformation {
                    label: lsp::ParameterLabel::Simple(
                        "$bucket".to_string(),
                    ),
                    documentation: None,
                }]),
                active_parameter: None,
            };

        assert_eq!(64, result.signatures.len());
        assert_eq!(None, result.active_signature);
        assert_eq!(None, result.active_parameter);
        assert_eq!(
            1,
            result
                .signatures
                .into_iter()
                .filter(|x| *x == expected_signature_information)
                .collect::<Vec<lsp::SignatureInformation>>()
                .len()
        );
    }
}
