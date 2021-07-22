use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use flux::semantic::walk;
use log::{debug, error, info, warn};
use lspower::jsonrpc::Result;
use lspower::lsp;
use lspower::LanguageServer;

use crate::handlers::document_symbol::sort_symbols;
use crate::handlers::find_node;
use crate::handlers::signature_help::find_stdlib_signatures;
use crate::visitors::semantic::{
    DefinitionFinderVisitor, FoldFinderVisitor, NodeFinderVisitor,
    SymbolsVisitor,
};

// The spec talks specifically about setting versions for files, but isn't
// clear on how those versions are surfaced to the client, if ever. This
// type could be extended to keep track of versions of files, but simplicity
// is preferred at this point.
type FileStore = Arc<Mutex<HashMap<lsp::Url, String>>>;

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

/// Take a lsp::Range that contains a start and end lsp::Position, find the
/// indexes of those points in the string, and replace that range with a new string.
fn replace_string_in_range(
    mut contents: String,
    range: lsp::Range,
    new: String,
) -> String {
    let mut string_range: (usize, usize) = (0, 0);
    let lookup = line_col::LineColLookup::new(&contents);
    for i in 0..contents.len() {
        let linecol = lookup.get(i);
        if string_range.0 == 0 {
            if linecol.0 == (range.start.line as usize)
                && linecol.1 == (range.start.character as usize)
            {
                string_range.0 = i;
            }
        } else if linecol.0 == (range.end.line as usize)
            && linecol.1 == (range.end.character as usize)
        {
            string_range.1 = i + 1; // Range is not inclusive.
            break;
        }
    }
    if string_range.1 == 0 {
        error!("range end not found after range start");
        return contents;
    }
    contents.replace_range(string_range.0..string_range.1, &new);
    contents
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
    store: FileStore,
    options: LspServerOptions,
}

impl LspServer {
    pub fn disable_folding(self) -> Self {
        Self {
            store: self.store,
            options: LspServerOptions {
                folding: false,
                influxdb_url: self.options.influxdb_url,
                token: self.options.token,
                org: self.options.org,
            },
        }
    }
    pub fn with_influxdb_url(self, influxdb_url: String) -> Self {
        Self {
            store: self.store,
            options: LspServerOptions {
                folding: self.options.folding,
                influxdb_url: Some(influxdb_url),
                token: self.options.token,
                org: self.options.org,
            },
        }
    }
    pub fn with_token(self, token: String) -> Self {
        Self {
            store: self.store,
            options: LspServerOptions {
                folding: self.options.folding,
                influxdb_url: self.options.influxdb_url,
                token: Some(token),
                org: self.options.org,
            },
        }
    }
    pub fn with_org(self, org: String) -> Self {
        Self {
            store: self.store,
            options: LspServerOptions {
                folding: self.options.folding,
                influxdb_url: self.options.influxdb_url,
                token: self.options.token,
                org: Some(org),
            },
        }
    }
}

impl Default for LspServer {
    fn default() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
            options: LspServerOptions {
                folding: true,
                influxdb_url: None,
                token: None,
                org: None,
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
                definition_provider: None,
                document_formatting_provider: None,
                document_highlight_provider: None,
                document_link_provider: None,
                document_on_type_formatting_provider: None,
                document_range_formatting_provider: None,
                document_symbol_provider: None,
                execute_command_provider: None,
                experimental: None,
                folding_range_provider: Some(
                    lsp::FoldingRangeProviderCapability::Simple(true),
                ),
                hover_provider: Some(
                    lsp::HoverProviderCapability::Simple(true),
                ),
                implementation_provider: None,
                linked_editing_range_provider: None,
                moniker_provider: None,
                references_provider: None,
                rename_provider: None,
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
        let mut store = self.store.lock().unwrap();
        if store.contains_key(&key) {
            // The protocol spec is unclear on whether trying to open a file
            // that is already opened is allowed, and research would indicate that
            // there are badly behaved clients that do this. Rather than making this
            // error, log the issue and move on.
            warn!("textDocument/didOpen called on open file {}", key);
        }
        store.insert(key, value);
    }
    async fn did_change(
        &self,
        params: lsp::DidChangeTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;
        let mut store = self.store.lock().unwrap();
        if !store.contains_key(&key) {
            error!(
                "textDocument/didChange called on unknown file {}",
                key
            );
            return;
        }
        for change in params.content_changes {
            if let Some(range) = change.range {
                let contents = store.get(&key).unwrap();
                let new_contents = replace_string_in_range(
                    contents.clone(),
                    range,
                    change.text,
                );
                store.insert(key.clone(), new_contents);
            } else {
                store.insert(key.clone(), change.text);
            }
        }
    }
    async fn did_close(
        &self,
        params: lsp::DidCloseTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;

        let mut store = self.store.lock().unwrap();
        if !store.contains_key(&key) {
            // The protocol spec is unclear on whether trying to close a file
            // that isn't open is allowed. To stop consistent with the
            // implementation of textDocument/didOpen, this error is logged and
            // allowed.
            warn!(
                "textDocument/didClose called on unknown file {}",
                key
            );
        }
        store.remove(&key);
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
    async fn formatting(
        &self,
        params: lsp::DocumentFormattingParams,
    ) -> Result<Option<Vec<lsp::TextEdit>>> {
        let key = params.text_document.uri;
        let store = self.store.lock().unwrap();
        if !store.contains_key(&key) {
            error!(
                "formatting failed: file {} not open on server",
                key
            );
            return Err(lspower::jsonrpc::Error::invalid_params(
                format!("file not opened: {}", key),
            ));
        }
        let contents = store.get(&key).unwrap();
        let mut formatted =
            flux::formatter::format(&contents).unwrap();
        if let Some(trim_trailing_whitespace) =
            params.options.trim_trailing_whitespace
        {
            if trim_trailing_whitespace {
                info!("textDocument/formatting requested trimming trailing whitespace, but the flux formatter will always trim trailing whitespace");
            }
        }
        if let Some(insert_final_newline) =
            params.options.insert_final_newline
        {
            if insert_final_newline
                && formatted.chars().nth(formatted.len() - 1).unwrap()
                    != '\n'
            {
                formatted.push('\n');
            }
        }
        if let Some(trim_final_newlines) =
            params.options.trim_final_newlines
        {
            if trim_final_newlines
                && formatted.chars().nth(formatted.len() - 1).unwrap()
                    != '\n'
            {
                info!("textDocument/formatting requested trimming final newlines, but the flux formatter will always trim trailing whitespace");
            }
        }
        let lookup = line_col::LineColLookup::new(formatted.as_str());
        let end = lookup.get(formatted.len() - 1);

        let edit = lsp::TextEdit::new(
            lsp::Range {
                start: lsp::Position {
                    line: 1,
                    character: 1,
                },
                end: lsp::Position {
                    line: end.0 as u32,
                    character: end.1 as u32,
                },
            },
            formatted,
        );

        Ok(Some(vec![edit]))
    }
    async fn folding_range(
        &self,
        params: lsp::FoldingRangeParams,
    ) -> Result<Option<Vec<lsp::FoldingRange>>> {
        let key = params.text_document.uri;
        let store = self.store.lock().unwrap();
        if !store.contains_key(&key) {
            error!(
                "formatting failed: file {} not open on server",
                key
            );
            return Err(lspower::jsonrpc::Error::invalid_params(
                format!("file not opened: {}", key),
            ));
        }
        let contents = store.get(&key).unwrap();
        let pkg = parse_and_analyze(contents.as_str());
        let mut visitor = FoldFinderVisitor::default();
        let pkg_node = walk::Node::Package(&pkg);

        walk::walk(&mut visitor, Rc::new(pkg_node));

        let state = visitor.state.borrow();
        let nodes = (*state).nodes.clone();

        let mut results = vec![];
        for node in nodes {
            results.push(lsp::FoldingRange {
                start_line: node.loc().start.line,
                start_character: Some(node.loc().start.column),
                end_line: node.loc().end.line,
                end_character: Some(node.loc().end.column),
                kind: Some(lsp::FoldingRangeKind::Region),
            })
        }

        Ok(Some(results))
    }
    async fn document_symbol(
        &self,
        params: lsp::DocumentSymbolParams,
    ) -> Result<Option<lsp::DocumentSymbolResponse>> {
        let key = params.text_document.uri;
        let store = self.store.lock().unwrap();
        if !store.contains_key(&key) {
            error!(
                "documentSymbol request failed: file {} not open on server",
                key,
            );
            return Err(lspower::jsonrpc::Error::invalid_params(
                format!("file not opened: {}", key),
            ));
        }

        let contents = store.get(&key).unwrap();
        let pkg = parse_and_analyze(contents);
        let pkg_node = walk::Node::Package(&pkg);
        let mut visitor = SymbolsVisitor::new(key);
        walk::walk(&mut visitor, Rc::new(pkg_node));

        let state = visitor.state.borrow();
        let mut symbols = (*state).symbols.clone();

        symbols.sort_by(|a, b| sort_symbols(a, b));

        let response = lsp::DocumentSymbolResponse::Flat(symbols);

        Ok(Some(response))
    }
    async fn goto_definition(
        &self,
        params: lsp::GotoDefinitionParams,
    ) -> Result<Option<lsp::GotoDefinitionResponse>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let store = self.store.lock().unwrap();
        if !store.contains_key(&key) {
            error!(
                "formatting failed: file {} not open on server",
                key
            );
            return Err(lspower::jsonrpc::Error::invalid_params(
                format!("file not opened: {}", key),
            ));
        }
        let contents = store.get(&key).unwrap();
        let pkg = parse_and_analyze(contents);
        let pkg_node = flux::semantic::walk::Node::Package(&pkg);
        let mut visitor = NodeFinderVisitor::new(
            params.text_document_position_params.position,
        );

        flux::semantic::walk::walk(&mut visitor, Rc::new(pkg_node));

        let state = visitor.state.borrow();
        let node = (*state).node.clone();
        let path = (*state).path.clone();

        if let Some(node) = node {
            let name = match node.as_ref() {
                flux::semantic::walk::Node::Identifier(ident) => {
                    Some(ident.name.clone())
                }
                flux::semantic::walk::Node::IdentifierExpr(ident) => {
                    Some(ident.name.clone())
                }
                _ => return Ok(None),
            };

            if let Some(node_name) = name {
                let path_iter = path.iter().rev();
                for n in path_iter {
                    match n.as_ref() {
                        flux::semantic::walk::Node::FunctionExpr(
                            _,
                        )
                        | flux::semantic::walk::Node::Package(_)
                        | flux::semantic::walk::Node::File(_) => {
                            if let flux::semantic::walk::Node::FunctionExpr(f) = n.as_ref() {
                                for param in f.params.clone() {
                                    let name = param.key.name;
                                    if name != node_name {
                                        continue;
                                    }
                                    let location = {
                                        let start = lsp::Position {
                                            line: node.loc().start.line - 1,
                                            character: node.loc().start.column - 1,
                                        };

                                        let end = lsp::Position {
                                            line: node.loc().end.line - 1,
                                            character: node.loc().end.column - 1,
                                        };

                                        let range = lsp::Range { start, end };

                                        lsp::Location { uri: key, range }
                                    };
                                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(location)));
                                }
                            }

                            let mut definition_visitor: DefinitionFinderVisitor =
                                DefinitionFinderVisitor::new(node_name.clone());

                            flux::semantic::walk::walk(
                                &mut definition_visitor,
                                n.clone(),
                            );

                            let state =
                                definition_visitor.state.borrow();
                            if let Some(node) = state.node.clone() {
                                let location = {
                                    let start_line =
                                        node.loc().start.line;
                                    let start_col =
                                        node.loc().start.column;
                                    let end_line =
                                        node.loc().end.line;
                                    let end_col =
                                        node.loc().end.column;

                                    lsp::Location {
                                        uri: key,
                                        range: lsp::Range {
                                            start: lsp::Position {
                                                line: start_line,
                                                character: start_col,
                                            },
                                            end: lsp::Position {
                                                line: end_line,
                                                character: end_col,
                                            },
                                        },
                                    }
                                };
                                return Ok(Some(lsp::GotoDefinitionResponse::Scalar(location)));
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use std::collections::HashMap;

    use lspower::lsp;
    use lspower::LanguageServer;
    use tokio_test::block_on;

    use super::LspServer;

    #[allow(dead_code)]
    const SIGNATURE_HELP: &'static str =
        include_str!("../tests/fixtures/signatures.flux");

    fn create_server() -> LspServer {
        LspServer::default()
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

        block_on(server.did_open(params));

        assert_eq!(
            vec![&lsp::Url::parse("file:///home/user/file.flux")
                .unwrap()],
            server
                .store
                .lock()
                .unwrap()
                .keys()
                .collect::<Vec<&lsp::Url>>()
        );
        let uri =
            lsp::Url::parse("file:///home/user/file.flux").unwrap();
        let contents =
            server.store.lock().unwrap().get(&uri).unwrap().clone();
        assert_eq!("from(", contents);
    }

    #[test]
    fn test_did_change() {
        let server = create_server();
        open_file(
            &server,
            r#"from(bucket: "bucket") |> first()"#.to_string(),
        );

        let params = lsp::DidChangeTextDocumentParams {
            text_document: lsp::VersionedTextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                version: -2,
            },
            content_changes: vec![
                lsp::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: r#"from(bucket: "bucket")"#.to_string(),
                },
            ],
        };

        block_on(server.did_change(params));

        let uri =
            lsp::Url::parse("file:///home/user/file.flux").unwrap();
        let contents =
            server.store.lock().unwrap().get(&uri).unwrap().clone();
        assert_eq!(r#"from(bucket: "bucket")"#, contents);
    }

    #[test]
    fn test_did_change_with_range() {
        let server = create_server();
        open_file(
            &server,
            r#"from(bucket: "bucket")
|> last()"#
                .to_string(),
        );

        let params = lsp::DidChangeTextDocumentParams {
            text_document: lsp::VersionedTextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                version: -2,
            },
            content_changes: vec![
                lsp::TextDocumentContentChangeEvent {
                    range: Some(lsp::Range {
                        start: lsp::Position {
                            line: 2,
                            character: 4,
                        },
                        end: lsp::Position {
                            line: 2,
                            character: 9,
                        },
                    }),
                    range_length: None,
                    text: r#" first()"#.to_string(),
                },
            ],
        };

        block_on(server.did_change(params));

        let uri =
            lsp::Url::parse("file:///home/user/file.flux").unwrap();
        let contents =
            server.store.lock().unwrap().get(&uri).unwrap().clone();
        assert_eq!(
            r#"from(bucket: "bucket")
|>  first()"#,
            contents
        );
    }

    #[test]
    fn test_did_change_with_multiline_range() {
        let server = create_server();
        open_file(
            &server,
            r#"from(bucket: "bucket")
|> group()
|> last()"#
                .to_string(),
        );

        let params = lsp::DidChangeTextDocumentParams {
            text_document: lsp::VersionedTextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                version: -2,
            },
            content_changes: vec![
                lsp::TextDocumentContentChangeEvent {
                    range: Some(lsp::Range {
                        start: lsp::Position {
                            line: 2,
                            character: 3,
                        },
                        end: lsp::Position {
                            line: 3,
                            character: 8,
                        },
                    }),
                    range_length: None,
                    text: r#"drop(columns: ["_start", "_stop"])
|>  first( "#
                        .to_string(),
                },
            ],
        };

        block_on(server.did_change(params));

        let uri =
            lsp::Url::parse("file:///home/user/file.flux").unwrap();
        let contents =
            server.store.lock().unwrap().get(&uri).unwrap().clone();
        assert_eq!(
            r#"from(bucket: "bucket")
|>drop(columns: ["_start", "_stop"])
|>  first( )"#,
            contents
        );
    }

    #[test]
    fn test_did_close() {
        let server = create_server();
        open_file(&server, "from(".to_string());

        assert!(server.store.lock().unwrap().keys().next().is_some());

        let params = lsp::DidCloseTextDocumentParams {
            text_document: lsp::TextDocumentIdentifier::new(
                lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            ),
        };

        block_on(server.did_close(params));

        assert!(server.store.lock().unwrap().keys().next().is_none());
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

    // If the file hasn't been opened on the server, return an error.
    #[test]
    fn test_formatting_not_opened() {
        let server = create_server();

        let params = lsp::DocumentFormattingParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file::///home/user/file.flux")
                    .unwrap(),
            },
            options: lsp::FormattingOptions {
                tab_size: 0,
                insert_spaces: false,
                properties:
                    HashMap::<String, lsp::FormattingProperty>::new(),
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };
        let result = block_on(server.formatting(params));

        assert!(result.is_err());
    }

    #[test]
    fn test_formatting() {
        let fluxscript = r#"
import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
      |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
      |> group(columns:["env", "error"])
    |> count()
  |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string());

        let params = lsp::DocumentFormattingParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            options: lsp::FormattingOptions {
                tab_size: 0,
                insert_spaces: false,
                properties:
                    HashMap::<String, lsp::FormattingProperty>::new(),
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };
        let result =
            block_on(server.formatting(params)).unwrap().unwrap();

        let expected = lsp::TextEdit::new(
            lsp::Range {
                start: lsp::Position {
                    line: 1,
                    character: 1,
                },
                end: lsp::Position {
                    line: 15,
                    character: 96,
                },
            },
            flux::formatter::format(&fluxscript).unwrap(),
        );
        assert_eq!(vec![expected], result);
    }

    #[test]
    fn test_formatting_insert_final_newline() {
        let fluxscript = r#"
import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
      |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
      |> group(columns:["env", "error"])
    |> count()
  |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))

"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string());

        let params = lsp::DocumentFormattingParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            options: lsp::FormattingOptions {
                tab_size: 0,
                insert_spaces: false,
                properties:
                    HashMap::<String, lsp::FormattingProperty>::new(),
                trim_trailing_whitespace: None,
                insert_final_newline: Some(true),
                trim_final_newlines: None,
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };
        let result =
            block_on(server.formatting(params)).unwrap().unwrap();

        let mut formatted_text =
            flux::formatter::format(&fluxscript).unwrap();
        formatted_text.push('\n');
        let expected = lsp::TextEdit::new(
            lsp::Range {
                start: lsp::Position {
                    line: 1,
                    character: 1,
                },
                // This reads funny, because line 15 is only 96 characters long.
                // Character number 97 is a newline, but it doesn't show as line
                // 16 because there aren't any characters on the line, and we
                // can't uso character 0 there.
                end: lsp::Position {
                    line: 15,
                    character: 97,
                },
            },
            formatted_text,
        );
        assert_eq!(vec![expected], result);
    }

    #[test]
    fn test_folding_not_opened() {
        let server = create_server();

        let params = lsp::FoldingRangeParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result = block_on(server.folding_range(params));

        assert!(result.is_err());
    }

    #[test]
    fn test_folding() {
        let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string());

        let params = lsp::FoldingRangeParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result =
            block_on(server.folding_range(params)).unwrap().unwrap();

        let expected = vec![
            lsp::FoldingRange {
                start_line: 6,
                start_character: Some(26),
                end_line: 9,
                end_character: Some(38),
                kind: Some(lsp::FoldingRangeKind::Region),
            },
            lsp::FoldingRange {
                start_line: 15,
                start_character: Some(26),
                end_line: 15,
                end_character: Some(96),
                kind: Some(lsp::FoldingRangeKind::Region),
            },
        ];

        assert_eq!(expected, result);
    }

    #[test]
    fn test_document_symbol_not_opened() {
        let server = create_server();

        let params = lsp::DocumentSymbolParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result = block_on(server.document_symbol(params));

        assert!(result.is_err());
    }

    #[test]
    fn test_document_symbol() {
        let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string());

        let params = lsp::DocumentSymbolParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };
        let symbol_response =
            block_on(server.document_symbol(params))
                .unwrap()
                .unwrap();

        match symbol_response {
            lsp::DocumentSymbolResponse::Flat(symbols) => {
                assert_eq!(symbols.len(), 38)
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_goto_definition_not_opened() {
        let server = create_server();

        let params = lsp::GotoDefinitionParams {
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
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result = block_on(server.goto_definition(params));

        assert!(result.is_err());
    }

    #[test]
    fn test_goto_definition() {
        let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string());

        let params = lsp::GotoDefinitionParams {
            text_document_position_params:
                lsp::TextDocumentPositionParams::new(
                    lsp::TextDocumentIdentifier::new(
                        lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    ),
                    lsp::Position::new(8, 35),
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result = block_on(server.goto_definition(params))
            .unwrap()
            .unwrap();

        let expected =
            lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 2,
                        character: 1,
                    },
                    end: lsp::Position {
                        line: 2,
                        character: 25,
                    },
                },
            });

        assert_eq!(expected, result);
    }
}
