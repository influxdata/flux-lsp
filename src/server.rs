use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, Mutex},
};

use anyhow::Result;

use flux::semantic::{
    nodes::FunctionParameter, nodes::Symbol, types::MonoType, walk,
};

use lspower::{
    jsonrpc::Error as RpcError, jsonrpc::ErrorCode as RpcErrorCode,
    jsonrpc::Result as RpcResult, lsp, Client, LanguageServer,
};

use crate::{
    completion, convert, shared::FunctionSignature, stdlib,
    visitors::semantic,
};

// The spec talks specifically about setting versions for files, but isn't
// clear on how those versions are surfaced to the client, if ever. This
// type could be extended to keep track of versions of files, but simplicity
// is preferred at this point.
type FileStore = Arc<Mutex<HashMap<lsp::Url, String>>>;

/// Returns `None` when the flux source fails analysis.
fn parse_and_analyze(
    code: &str,
) -> Result<Option<flux::semantic::nodes::Package>> {
    let mut analyzer = flux::new_semantic_analyzer(
        flux::semantic::AnalyzerConfig {
            // Explicitly disable the AST and Semantic checks.
            // We do not care if the code is syntactically or semantically correct as this may be
            // partially written code.
            skip_checks: true,
        },
    )?;
    let (_, sem_pkg) = match analyzer.analyze_source(
        "".to_string(),
        "main.flux".to_string(),
        code,
    ) {
        Ok(res) => res,
        Err(e) => {
            log::debug!("Unable to parse source: {}", e);
            return Ok(None);
        }
    };
    Ok(Some(sem_pkg))
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
        if linecol.0 == (range.start.line as usize) + 1
            && linecol.1 == (range.start.character as usize) + 1
        {
            string_range.0 = i;
        }
        if linecol.0 == (range.end.line as usize) + 1
            && linecol.1 == (range.end.character as usize) + 1
        {
            string_range.1 = i + 1; // Range is not inclusive.
            break;
        }
    }
    if string_range.1 < string_range.0 {
        log::error!("range end not found after range start");
        return contents;
    }
    contents.replace_range(string_range.0..string_range.1, &new);
    contents
}

fn function_defines(
    name: &str,
    params: &[FunctionParameter],
) -> bool {
    params.iter().any(|param| param.key.name == name)
}

fn is_scope(name: &Symbol, n: walk::Node<'_>) -> bool {
    let mut dvisitor =
        semantic::DefinitionFinderVisitor::new(name.clone());
    walk::walk(&mut dvisitor, n);

    dvisitor.node.is_some()
}

fn find_references(
    uri: &lsp::Url,
    result: NodeFinderResult,
) -> Vec<lsp::Location> {
    if let Some(node) = result.node {
        let name = match node {
            walk::Node::Identifier(ident) => &ident.name,
            walk::Node::IdentifierExpr(ident) => &ident.name,
            _ => return Vec::new(),
        };

        let mut path_iter = result.path.iter().rev();
        let scope: walk::Node =
            match path_iter.find_map(|n| match n {
                walk::Node::FunctionExpr(f)
                    if function_defines(name, &f.params) =>
                {
                    Some(n)
                }
                walk::Node::Package(_) | walk::Node::File(_)
                    if is_scope(name, n.clone()) =>
                {
                    Some(n)
                }
                _ => None,
            }) {
                Some(n) => n.to_owned(),
                None => return Vec::new(),
            };

        let mut visitor =
            semantic::IdentFinderVisitor::new(name.clone());
        walk::walk(&mut visitor, scope);

        let locations: Vec<lsp::Location> = visitor
            .identifiers
            .iter()
            .map(|node| convert::node_to_location(node, uri.clone()))
            .collect();
        locations
    } else {
        Vec::new()
    }
}

fn create_signature_information(
    fs: FunctionSignature,
) -> lsp::SignatureInformation {
    lsp::SignatureInformation {
        label: fs.create_signature(),
        parameters: Some(fs.create_parameters()),
        documentation: None,
        active_parameter: None,
    }
}

pub fn find_stdlib_signatures(
    name: &str,
    package: &str,
) -> Vec<lsp::SignatureInformation> {
    stdlib::get_stdlib_functions()
        .into_iter()
        .filter(|x| x.name == name && x.package_name == package)
        .map(|x| {
            x.signatures()
                .into_iter()
                .map(create_signature_information)
        })
        .fold(vec![], |mut acc, x| {
            acc.extend(x);
            acc
        })
}

#[derive(Clone)]
struct LspServerOptions {
    folding: bool,
}

pub struct LspServerBuilder {
    options: LspServerOptions,
}

impl LspServerBuilder {
    pub fn disable_folding(self) -> Self {
        Self {
            options: LspServerOptions { folding: false },
        }
    }

    pub fn build(self, client: Option<Client>) -> LspServer {
        LspServer::new(client, self.options)
    }
}

impl Default for LspServerBuilder {
    fn default() -> Self {
        LspServerBuilder {
            options: LspServerOptions { folding: true },
        }
    }
}

#[allow(dead_code)]
pub struct LspServer {
    client: Arc<Mutex<Option<Client>>>,
    store: FileStore,
    options: LspServerOptions,
}

impl LspServer {
    fn new(
        client: Option<Client>,
        options: LspServerOptions,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            store: Arc::new(Mutex::new(HashMap::new())),
            options,
        }
    }

    // Get the client from out of its arc and mutex.
    // Note the lspower::Client has a cheap clone method to make it easy
    // to pass around many instances of the client.
    //
    // We leverage that here so we do not have to keep a lock or
    // an extra reference to the client.
    fn get_client(&self) -> Option<Client> {
        match self.client.lock() {
            Ok(client) => (*client).clone(),
            Err(err) => {
                log::error!("failed to get lock on client: {}", err);
                None
            }
        }
    }

    fn get_document(&self, key: &lsp::Url) -> RpcResult<String> {
        let store = match self.store.lock() {
            Ok(value) => value,
            Err(err) => {
                return Err(lspower::jsonrpc::Error {
                    code: lspower::jsonrpc::ErrorCode::InternalError,
                    message: format!(
                        "Could not acquire store lock. Error: {}",
                        err
                    ),
                    data: None,
                });
            }
        };
        if let Some(contents) = store.get(key) {
            Ok(contents.clone())
        } else {
            Err(lspower::jsonrpc::Error::invalid_params(format!(
                "file not opened: {}",
                key
            )))
        }
    }

    /// Returns `None` when the flux source couldn't be analyzed.
    fn parse_analyze_document(
        &self,
        key: &lsp::Url,
    ) -> RpcResult<Option<flux::semantic::nodes::Package>> {
        let contents = self.get_document(key)?;
        match parse_and_analyze(&contents) {
            Ok(maybe_pkg) => Ok(maybe_pkg),
            Err(err) => RpcResult::Err(RpcError {
                code: RpcErrorCode::InternalError,
                message: format!("{}", err),
                data: None,
            }),
        }
    }

    // Publish any diagnostics to the client
    async fn publish_diagnostics(&self, key: &lsp::Url, text: &str) {
        // If we have a client back to the editor report any diagnostics found in the document
        if let Some(client) = self.get_client() {
            match self.compute_diagnostics(key, text) {
                Ok(diagnostics) => {
                    client
                        .publish_diagnostics(
                            key.clone(),
                            diagnostics,
                            None,
                        )
                        .await
                }
                // TODO(nathanielc): Report errors creating the analyzer to the client
                Err(e) => log::error!(
                    "failed to compute diagnostics: {}",
                    e
                ),
            };
        };
    }

    fn compute_diagnostics(
        &self,
        key: &lsp::Url,
        text: &str,
    ) -> Result<Vec<lsp::Diagnostic>> {
        match flux::new_semantic_analyzer(
            flux::semantic::AnalyzerConfig::default(),
        ) {
            Ok(mut analyzer) => {
                match analyzer.analyze_source(
                    "".to_string(),
                    key.to_string(),
                    text,
                ) {
                    // Send back empty list of diagnostics,
                    // this is important as the client needs to
                    // explicitly know that all previous diagnostics
                    // are no longer relevant.
                    Ok(_) => Ok(Vec::new()),
                    Err(errors) => Ok(errors
                        .errors
                        .iter()
                        .map(|e| lsp::Diagnostic {
                            range: convert::ast_to_lsp_range(
                                &e.location,
                            ),
                            severity: Some(
                                lsp::DiagnosticSeverity::ERROR,
                            ),
                            source: Some("flux".to_string()),
                            message: e.error.to_string(),
                            ..lsp::Diagnostic::default()
                        })
                        .collect()),
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[lspower::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(
        &self,
        _: lsp::InitializeParams,
    ) -> RpcResult<lsp::InitializeResult> {
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
                definition_provider: Some(lsp::OneOf::Left(true)),
                document_formatting_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                document_highlight_provider: Some(lsp::OneOf::Left(
                    true,
                )),
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
                        self.options.folding,
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
                        lsp::TextDocumentSyncKind::FULL,
                    ),
                ),
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

    async fn shutdown(&self) -> RpcResult<()> {
        let mut client = match self.client.lock() {
            Ok(client) => client,
            Err(err) => {
                return RpcResult::Err(RpcError {
                    code: RpcErrorCode::InternalError,
                    message: format!("{}", err),
                    data: None,
                })
            }
        };
        // XXX(nathanielc): Replace the original client with None causing the original to be dropped.
        // Dropping the client will close its channel allowing the receiving end
        // to observe the end of the stream.
        // See PR for simple change to lspower that will simplify this logic https://github.com/silvanshade/lspower/pull/20
        *client = None;
        Ok(())
    }

    async fn did_open(
        &self,
        params: lsp::DidOpenTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;
        let value = params.text_document.text;
        self.publish_diagnostics(&key, value.as_str()).await;
        // Add document to the store
        let mut store = match self.store.lock() {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Could not acquire store lock. Error: {}",
                    err
                );
                return;
            }
        };
        match store.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
            Entry::Occupied(entry) => {
                // The protocol spec is unclear on whether trying to open a file
                // that is already opened is allowed, and research would indicate that
                // there are badly behaved clients that do this. Rather than making this
                // error, log the issue and move on.
                log::warn!(
                    "textDocument/didOpen called on open file {}",
                    entry.key(),
                );
            }
        }
    }

    async fn did_change(
        &self,
        params: lsp::DidChangeTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;
        let contents = {
            let mut store = match self.store.lock() {
                Ok(value) => value,
                Err(err) => {
                    log::warn!(
                        "Could not acquire store lock. Error: {}",
                        err
                    );
                    return;
                }
            };
            let mut contents = if let Some(contents) = store.get(&key)
            {
                Cow::Borrowed(contents)
            } else {
                log::error!(
                "textDocument/didChange called on unknown file {}",
                key
            );
                return;
            };
            for change in params.content_changes {
                contents =
                    Cow::Owned(if let Some(range) = change.range {
                        replace_string_in_range(
                            contents.into_owned(),
                            range,
                            change.text,
                        )
                    } else {
                        change.text
                    });
            }
            let new_contents = contents.into_owned();
            let c = new_contents.clone();
            store.insert(key.clone(), new_contents);
            c
        };
        self.publish_diagnostics(&key, contents.as_str()).await;
    }

    async fn did_save(
        &self,
        params: lsp::DidSaveTextDocumentParams,
    ) -> () {
        if let Some(text) = params.text {
            let key = params.text_document.uri;
            {
                let mut store = match self.store.lock() {
                    Ok(value) => value,
                    Err(err) => {
                        log::warn!(
                            "Could not acquire store lock. Error: {}",
                            err
                        );
                        return;
                    }
                };
                if !store.contains_key(&key) {
                    log::warn!(
                    "textDocument/didSave called on unknown file {}",
                    key
                );
                    return;
                }
                store.insert(key.clone(), text.clone());
            }
            self.publish_diagnostics(&key, text.as_str()).await;
        }
    }

    async fn did_close(
        &self,
        params: lsp::DidCloseTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;

        let mut store = match self.store.lock() {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Could not acquire store lock. Error: {}",
                    err
                );
                return;
            }
        };
        if store.remove(&key).is_none() {
            // The protocol spec is unclear on whether trying to close a file
            // that isn't open is allowed. To stop consistent with the
            // implementation of textDocument/didOpen, this error is logged and
            // allowed.
            log::warn!(
                "textDocument/didClose called on unknown file {}",
                key
            );
        }
    }

    async fn signature_help(
        &self,
        params: lsp::SignatureHelpParams,
    ) -> RpcResult<Option<lsp::SignatureHelp>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let mut signatures = vec![];
        let node_finder_result = find_node(
            walk::Node::Package(&pkg),
            params.text_document_position_params.position,
        );

        if let Some(node) = node_finder_result.node {
            if let walk::Node::CallExpr(call) = node {
                let callee = call.callee.clone();

                if let flux::semantic::nodes::Expression::Member(member) = callee.clone() {
                    let name = member.property.clone();
                    if let flux::semantic::nodes::Expression::Identifier(ident) = member.object.clone() {
                        signatures.extend(find_stdlib_signatures(&name, &ident.name));
                    }
                } else if let flux::semantic::nodes::Expression::Identifier(ident) = callee {
                    signatures.extend(find_stdlib_signatures(
                        &ident.name,
                        "builtin",
                    ));
                    // XXX: rockstar (13 Jul 2021) - Add support for user defined
                    // signatures.
                } else {
                    log::debug!("signature_help on non-member and non-identifier");
                }
            } else {
                log::debug!("signature_help on non-call expression");
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
    ) -> RpcResult<Option<Vec<lsp::TextEdit>>> {
        let key = params.text_document.uri;

        let contents = self.get_document(&key)?;
        let mut formatted = match flux::formatter::format(&contents) {
            Ok(value) => value,
            Err(err) => {
                return Err(lspower::jsonrpc::Error {
                    code: lspower::jsonrpc::ErrorCode::InternalError,
                    message: format!(
                        "Error formatting document: {}",
                        err
                    ),
                    data: None,
                })
            }
        };
        if let Some(trim_trailing_whitespace) =
            params.options.trim_trailing_whitespace
        {
            if trim_trailing_whitespace {
                log::info!("textDocument/formatting requested trimming trailing whitespace, but the flux formatter will always trim trailing whitespace");
            }
        }
        if let Some(insert_final_newline) =
            params.options.insert_final_newline
        {
            if insert_final_newline
                && formatted.chars().last().unwrap_or(' ') != '\n'
            {
                formatted.push('\n');
            }
        }
        if let Some(trim_final_newlines) =
            params.options.trim_final_newlines
        {
            if trim_final_newlines
                && formatted.chars().last().unwrap_or(' ') != '\n'
            {
                log::info!("textDocument/formatting requested trimming final newlines, but the flux formatter will always trim trailing whitespace");
            }
        }

        // The new text shows the range of the previously replaced section,
        // not the range of the new section.
        let lookup = line_col::LineColLookup::new(contents.as_str());
        let end = lookup.get(contents.len());

        let edit = lsp::TextEdit::new(
            lsp::Range {
                start: lsp::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp::Position {
                    line: (end.0 - 1) as u32,
                    character: (end.1 - 1) as u32,
                },
            },
            formatted,
        );

        Ok(Some(vec![edit]))
    }

    async fn folding_range(
        &self,
        params: lsp::FoldingRangeParams,
    ) -> RpcResult<Option<Vec<lsp::FoldingRange>>> {
        let key = params.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let mut visitor = semantic::FoldFinderVisitor::default();
        let pkg_node = walk::Node::Package(&pkg);

        walk::walk(&mut visitor, pkg_node);

        let nodes = visitor.nodes;

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
    ) -> RpcResult<Option<lsp::DocumentSymbolResponse>> {
        let key = params.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let pkg_node = walk::Node::Package(&pkg);
        let mut visitor = semantic::SymbolsVisitor::new(key);
        walk::walk(&mut visitor, pkg_node);

        let mut symbols = visitor.symbols;

        symbols.sort_by(|a, b| {
            let a_start = a.location.range.start;
            let b_start = b.location.range.start;

            if a_start.line == b_start.line {
                a_start.character.cmp(&b_start.character)
            } else {
                a_start.line.cmp(&b_start.line)
            }
        });

        let response = lsp::DocumentSymbolResponse::Flat(symbols);

        Ok(Some(response))
    }

    async fn goto_definition(
        &self,
        params: lsp::GotoDefinitionParams,
    ) -> RpcResult<Option<lsp::GotoDefinitionResponse>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let pkg_node = walk::Node::Package(&pkg);
        let mut visitor = semantic::NodeFinderVisitor::new(
            params.text_document_position_params.position,
        );

        flux::semantic::walk::walk(&mut visitor, pkg_node.clone());

        if let Some(node) = visitor.node {
            let node_name = match node {
                walk::Node::Identifier(ident) => &ident.name,
                walk::Node::IdentifierExpr(ident) => &ident.name,
                _ => return Ok(None),
            };

            let mut definition_visitor =
                semantic::DefinitionFinderVisitor::new(
                    node_name.clone(),
                );
            flux::semantic::walk::walk(
                &mut definition_visitor,
                pkg_node,
            );

            if let Some(node) = definition_visitor.node {
                let location = convert::node_to_location(&node, key);
                return Ok(Some(lsp::GotoDefinitionResponse::from(
                    location,
                )));
            }
        }
        Ok(None)
    }

    async fn rename(
        &self,
        params: lsp::RenameParams,
    ) -> RpcResult<Option<lsp::WorkspaceEdit>> {
        let key = params.text_document_position.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let node = find_node(
            walk::Node::Package(&pkg),
            params.text_document_position.position,
        );

        let locations = find_references(&key, node);
        let edits = locations
            .iter()
            .map(|location| lsp::TextEdit {
                range: location.range,
                new_text: params.new_name.clone(),
            })
            .collect::<Vec<lsp::TextEdit>>();

        let mut changes = HashMap::new();
        changes.insert(key, edits);

        let response = lsp::WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        };
        Ok(Some(response))
    }

    async fn document_highlight(
        &self,
        params: lsp::DocumentHighlightParams,
    ) -> RpcResult<Option<Vec<lsp::DocumentHighlight>>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let node = find_node(
            walk::Node::Package(&pkg),
            params.text_document_position_params.position,
        );

        let refs = find_references(&key, node);
        Ok(Some(
            refs.iter()
                .map(|r| lsp::DocumentHighlight {
                    kind: Some(lsp::DocumentHighlightKind::TEXT),

                    range: r.range,
                })
                .collect(),
        ))
    }

    async fn references(
        &self,
        params: lsp::ReferenceParams,
    ) -> RpcResult<Option<Vec<lsp::Location>>> {
        let key = params.text_document_position.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let node = find_node(
            walk::Node::Package(&pkg),
            params.text_document_position.position,
        );

        Ok(Some(find_references(&key, node)))
    }

    async fn hover(
        &self,
        params: lsp::HoverParams,
    ) -> RpcResult<Option<lsp::Hover>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let maybe_pkg = self.parse_analyze_document(&key)?;
        let pkg = match maybe_pkg {
            Some(pkg) => pkg,
            // Short circuit if the flux source couldn't be analyzed
            None => return Ok(None),
        };

        let node_finder_result = find_node(
            walk::Node::Package(&pkg),
            params.text_document_position_params.position,
        );
        if let Some(node) = node_finder_result.node {
            let path = &node_finder_result.path;
            let hover_type = node.type_of().or_else(|| match node {
                walk::Node::Identifier(ident) => {
                    // We hovered over an identifier without an attached type, try to figure
                    // it out from its context
                    let parent = path.get(path.len() - 2)?;
                    match parent {
                        // The type of assigned variables is the type of the right hand side
                        walk::Node::VariableAssgn(var) => {
                            Some(var.init.type_of())
                        }
                        walk::Node::MemberAssgn(var) => {
                            Some(var.init.type_of())
                        }
                        walk::Node::BuiltinStmt(builtin) => {
                            Some(builtin.typ_expr.expr.clone())
                        }

                        // The type Function parameters can be derived from the function type
                        // stored in the function expression
                        walk::Node::FunctionParameter(_) => {
                            let func = path.get(path.len() - 3)?;
                            match func {
                                walk::Node::FunctionExpr(func) => {

                                    // TODO Use MonoType::parameter directly
                                    let field = ident.name.as_str();
                                    match &func.typ {
                                        MonoType::Fun(f) => f.req.get(field).or_else(|| f.opt.get(field)).or_else(|| {
                                            f.pipe
                                                .as_ref()
                                                .and_then(|pipe| if pipe.k == field { Some(&pipe.v) } else { None })
                                        })

                                                .cloned()
                                        ,
                                        _ => None,
                                    }
                                }
                                _ => None
                            }
                        }
                        _ => None,
                    }
                },
                _ => None,
            });
            if let Some(typ) = hover_type {
                return Ok(Some(lsp::Hover {
                    contents: lsp::HoverContents::Scalar(
                        lsp::MarkedString::String(format!(
                            "type: {}",
                            typ
                        )),
                    ),
                    range: None,
                }));
            }
        }
        Ok(None)
    }

    // XXX: rockstar (9 Aug 2021) - This implementation exists here *solely* for
    // compatibility with the previous server. This behavior is identical to it,
    // although very clearly kinda useless.
    async fn completion_resolve(
        &self,
        params: lsp::CompletionItem,
    ) -> RpcResult<lsp::CompletionItem> {
        Ok(params)
    }

    async fn completion(
        &self,
        params: lsp::CompletionParams,
    ) -> RpcResult<Option<lsp::CompletionResponse>> {
        let key = &params.text_document_position.text_document.uri;

        let contents = self.get_document(key)?;

        let items = if let Some(ctx) = &params.context {
            match (ctx.trigger_kind, &ctx.trigger_character) {
                (
                    lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
                    Some(c),
                ) => match c.as_str() {
                    "." => completion::find_dot_completions(
                        params, &contents,
                    ),
                    ":" => {
                        // XXX: rockstar (29 Nov 2021) - This is where argument
                        // completion will live, e.g. buckets, measurements and
                        // tag keys/values. There are multiple issues open to support
                        // this functionality open currently.
                        Ok(lsp::CompletionList {
                            is_incomplete: false,
                            items: vec![],
                        })
                    }
                    "(" | "," => completion::find_param_completions(
                        Some(c),
                        &params,
                        contents.as_str(),
                    ),
                    _ => completion::find_completions(
                        params,
                        contents.as_str(),
                    ),
                },
                _ => completion::find_completions(
                    params,
                    contents.as_str(),
                ),
            }
        } else {
            completion::find_completions(params, contents.as_str())
        };

        let items = match items {
            Ok(items) => items,
            Err(e) => {
                log::warn!(
                    "failed to get completion items: {}",
                    e.msg
                );
                return Ok(None);
            }
        };

        let response = lsp::CompletionResponse::List(items);
        Ok(Some(response))
    }
}

#[derive(Default, Clone)]
struct NodeFinderResult<'a> {
    node: Option<flux::semantic::walk::Node<'a>>,
    path: Vec<flux::semantic::walk::Node<'a>>,
}

fn find_node(
    node: flux::semantic::walk::Node<'_>,
    position: lsp::Position,
) -> NodeFinderResult<'_> {
    let mut result = NodeFinderResult::default();
    let mut visitor = semantic::NodeFinderVisitor::new(position);

    flux::semantic::walk::walk(&mut visitor, node);

    result.node = visitor.node;
    result.path = visitor.path;

    result
}

// Url::to_file_path doesn't exist in wasm-unknown-unknown, for kinda
// obvious reasons. Ignore these tests when executing against that target.
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(deprecated)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use async_std::test;
    use expect_test::expect;
    use lspower::{lsp, LanguageServer};

    use super::*;

    /// Finds a `// ^` comment in `source` and returns the `lsp::Position` that the comment points
    /// at
    fn position_of(source: &str) -> lsp::Position {
        source.lines().enumerate().find_map(|(line, line_str)| {
            line_str.find("// ^").map(|j| lsp::Position {
                // The marker is on the line after the position we indicate
                line: line as u32 - 1,
                character: (line_str[..j].chars().count() + "// ^".len()) as u32,
            })
        }).unwrap_or_else(|| panic!("Could not find the position marker `// ^` in `{}`", source))
    }

    fn create_server() -> LspServer {
        let _ = env_logger::try_init();
        LspServerBuilder::default().build(None)
    }

    async fn open_file(server: &LspServer, text: String) {
        let params = lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem::new(
                lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                "flux".to_string(),
                1,
                text,
            ),
        };
        server.did_open(params).await;
    }

    #[test]
    async fn test_initialized() {
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

        let result = server.initialize(params).await.unwrap();
        let server_info = result.server_info.unwrap();

        assert_eq!(server_info.name, "flux-lsp".to_string());
        assert_eq!(server_info.version, Some("2.0".to_string()));
    }

    #[test]
    async fn test_shutdown() {
        let server = create_server();

        let result = server.shutdown().await.unwrap();

        assert_eq!((), result)
    }

    #[test]
    async fn test_did_open() {
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

        server.did_open(params).await;

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
    async fn test_did_change() {
        let server = create_server();
        open_file(
            &server,
            r#"from(bucket: "bucket") |> first()"#.to_string(),
        )
        .await;

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

        server.did_change(params).await;

        let uri =
            lsp::Url::parse("file:///home/user/file.flux").unwrap();
        let contents =
            server.store.lock().unwrap().get(&uri).unwrap().clone();
        assert_eq!(r#"from(bucket: "bucket")"#, contents);
    }

    #[test]
    async fn test_did_change_with_range() {
        let server = create_server();
        open_file(
            &server,
            r#"from(bucket: "bucket")
|> last()"#
                .to_string(),
        )
        .await;

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
                            line: 1,
                            character: 3,
                        },
                        end: lsp::Position {
                            line: 1,
                            character: 8,
                        },
                    }),
                    range_length: None,
                    text: r#" first()"#.to_string(),
                },
            ],
        };

        server.did_change(params).await;

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
    async fn test_did_change_with_multiline_range() {
        let server = create_server();
        open_file(
            &server,
            r#"from(bucket: "bucket")
|> group()
|> last()"#
                .to_string(),
        )
        .await;

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
                            line: 1,
                            character: 2,
                        },
                        end: lsp::Position {
                            line: 2,
                            character: 7,
                        },
                    }),
                    range_length: None,
                    text: r#"drop(columns: ["_start", "_stop"])
|>  first( "#
                        .to_string(),
                },
            ],
        };

        server.did_change(params).await;

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
    async fn test_did_save() {
        let server = create_server();
        open_file(
            &server,
            r#"from(bucket: "test") |> count()"#.to_string(),
        )
        .await;

        let uri =
            lsp::Url::parse("file:///home/user/file.flux").unwrap();

        let params = lsp::DidSaveTextDocumentParams {
            text_document: lsp::TextDocumentIdentifier::new(
                uri.clone(),
            ),
            text: Some(r#"from(bucket: "test2")"#.to_string()),
        };
        server.did_save(params).await;

        let contents =
            server.store.lock().unwrap().get(&uri).unwrap().clone();
        assert_eq!(r#"from(bucket: "test2")"#.to_string(), contents);
    }

    #[test]
    async fn test_did_close() {
        let server = create_server();
        open_file(&server, "from(".to_string()).await;

        assert!(server.store.lock().unwrap().keys().next().is_some());

        let params = lsp::DidCloseTextDocumentParams {
            text_document: lsp::TextDocumentIdentifier::new(
                lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            ),
        };

        server.did_close(params).await;

        assert!(server.store.lock().unwrap().keys().next().is_none());
    }

    // If the file hasn't been opened on the server get, return an error.
    #[test]
    async fn test_signature_help_not_opened() {
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
                    lsp::Position::new(0, 0),
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let result = server.signature_help(params).await;

        assert!(result.is_err());
    }

    #[test]
    async fn test_signature_help() {
        let server = create_server();
        open_file(&server, "from(".to_string()).await;

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
            server.signature_help(params).await.unwrap().unwrap();

        let expected_signature_labels: Vec<String> = vec![
            "from()",
            "from(bucket: $bucket)",
            "from(bucketID: $bucketID)",
            "from(host: $host)",
            "from(org: $org)",
            "from(orgID: $orgID)",
            "from(token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID)",
            "from(bucket: $bucket , host: $host)",
            "from(bucket: $bucket , org: $org)",
            "from(bucket: $bucket , orgID: $orgID)",
            "from(bucket: $bucket , token: $token)",
            "from(bucketID: $bucketID , host: $host)",
            "from(bucketID: $bucketID , org: $org)",
            "from(bucketID: $bucketID , orgID: $orgID)",
            "from(bucketID: $bucketID , token: $token)",
            "from(host: $host , org: $org)",
            "from(host: $host , orgID: $orgID)",
            "from(host: $host , token: $token)",
            "from(org: $org , orgID: $orgID)",
            "from(org: $org , token: $token)",
            "from(orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org)",
            "from(bucket: $bucket , bucketID: $bucketID , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , token: $token)",
            "from(bucket: $bucket , host: $host , org: $org)",
            "from(bucket: $bucket , host: $host , orgID: $orgID)",
            "from(bucket: $bucket , host: $host , token: $token)",
            "from(bucket: $bucket , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , org: $org , token: $token)",
            "from(bucket: $bucket , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , host: $host , org: $org)",
            "from(bucketID: $bucketID , host: $host , orgID: $orgID)",
            "from(bucketID: $bucketID , host: $host , token: $token)",
            "from(bucketID: $bucketID , org: $org , orgID: $orgID)",
            "from(bucketID: $bucketID , org: $org , token: $token)",
            "from(bucketID: $bucketID , orgID: $orgID , token: $token)",
            "from(host: $host , org: $org , orgID: $orgID)",
            "from(host: $host , org: $org , token: $token)",
            "from(host: $host , orgID: $orgID , token: $token)",
            "from(org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , host: $host , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , host: $host , org: $org , token: $token)",
            "from(bucket: $bucket , host: $host , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , org: $org , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , host: $host , org: $org , orgID: $orgID)",
            "from(bucketID: $bucketID , host: $host , org: $org , token: $token)",
            "from(bucketID: $bucketID , host: $host , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , org: $org , orgID: $orgID , token: $token)",
            "from(host: $host , org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , host: $host , org: $org , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , host: $host , org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org , orgID: $orgID , token: $token)",
        ].into_iter().map(|x| x.into()).collect::<Vec<String>>();

        assert_eq!(
            expected_signature_labels,
            result
                .signatures
                .iter()
                .map(|x| x.label.clone())
                .collect::<Vec<String>>()
        );
        assert_eq!(None, result.active_signature);
        assert_eq!(None, result.active_parameter);
    }

    // If the file hasn't been opened on the server, return an error.
    #[test]
    async fn test_formatting_not_opened() {
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
        let result = server.formatting(params).await;

        assert!(result.is_err());
    }

    #[test]
    async fn test_formatting() {
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
        open_file(&server, fluxscript.to_string()).await;

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
            server.formatting(params).await.unwrap().unwrap();

        let expected = lsp::TextEdit::new(
            lsp::Range {
                start: lsp::Position {
                    line: 0,
                    character: 0,
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
    async fn test_formatting_insert_final_newline() {
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
        open_file(&server, fluxscript.to_string()).await;

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
            server.formatting(params).await.unwrap().unwrap();

        let mut formatted_text =
            flux::formatter::format(&fluxscript).unwrap();
        formatted_text.push('\n');
        let expected = lsp::TextEdit::new(
            lsp::Range {
                start: lsp::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp::Position {
                    line: 17,
                    character: 0,
                },
            },
            formatted_text,
        );
        assert_eq!(vec![expected], result);
    }

    #[test]
    async fn test_folding_not_opened() {
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

        let result = server.folding_range(params).await;

        assert!(result.is_err());
    }

    #[test]
    async fn test_folding() {
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
        open_file(&server, fluxscript.to_string()).await;

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
            server.folding_range(params).await.unwrap().unwrap();

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
    async fn test_document_symbol_not_opened() {
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

        let result = server.document_symbol(params).await;

        assert!(result.is_err());
    }

    #[test]
    async fn test_document_symbol() {
        let expected_symbol_names: Vec<String> = vec![
            "strings",
            "env",
            "prod01-us-west-2",
            "errorCounts",
            "from",
            "bucket",
            "kube-infra/monthly",
            "range",
            "start",
            "filter",
            "fn",
            "r._measurement",
            "query_log",
            "r.error",
            "",
            "r._field",
            "responseSize",
            "r.env",
            "env",
            "group",
            "columns",
            "[]",
            "env",
            "error",
            "count",
            "group",
            "columns",
            "[]",
            "env",
            "_stop",
            "_start",
            "filter",
            "fn",
            "strings.containsStr",
            "v",
            "r.error",
            "substr",
            "AppendMappedRecordWithNulls",
        ]
        .into_iter()
        .map(|x| x.into())
        .collect::<Vec<String>>();

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
        open_file(&server, fluxscript.to_string()).await;

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
            server.document_symbol(params).await.unwrap().unwrap();

        match symbol_response {
            lsp::DocumentSymbolResponse::Flat(symbols) => {
                assert_eq!(
                    expected_symbol_names,
                    symbols
                        .iter()
                        .map(|x| x.name.clone())
                        .collect::<Vec<String>>()
                );
            }
            _ => unreachable!(),
        }
    }

    #[test]
    async fn test_goto_definition_not_opened() {
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

        let result = server.goto_definition(params).await;

        assert!(result.is_err());
    }

    #[test]
    async fn test_goto_definition() {
        let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
                                // ^
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::GotoDefinitionParams {
            text_document_position_params:
                lsp::TextDocumentPositionParams::new(
                    lsp::TextDocumentIdentifier::new(
                        lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    ),
                    position_of(fluxscript),
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result =
            server.goto_definition(params).await.unwrap().unwrap();

        let expected =
            lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 1,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 1,
                        character: 24,
                    },
                },
            });

        assert_eq!(expected, result);
    }

    #[test]
    async fn test_goto_definition_builtin() {
        let fluxscript = r#"
builtin func : (x: A) => A

func(x: 1)
// ^
"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::GotoDefinitionParams {
            text_document_position_params:
                lsp::TextDocumentPositionParams::new(
                    lsp::TextDocumentIdentifier::new(
                        lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    ),
                    position_of(fluxscript),
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result = server.goto_definition(params).await.unwrap();

        expect_test::expect![[r#"
            {
              "uri": "file:///home/user/file.flux",
              "range": {
                "start": {
                  "line": 1,
                  "character": 8
                },
                "end": {
                  "line": 1,
                  "character": 12
                }
              }
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }

    #[test]
    async fn test_goto_definition_shadowed() {
        let fluxscript = r#"
env = "prod01-us-west-2"

f = (env) => {
    return env
        // ^
}
"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::GotoDefinitionParams {
            text_document_position_params:
                lsp::TextDocumentPositionParams::new(
                    lsp::TextDocumentIdentifier::new(
                        lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    ),
                    position_of(fluxscript),
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result = server.goto_definition(params).await.unwrap();

        expect![[r#"
            {
              "uri": "file:///home/user/file.flux",
              "range": {
                "start": {
                  "line": 3,
                  "character": 5
                },
                "end": {
                  "line": 3,
                  "character": 8
                }
              }
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }

    #[test]
    async fn test_rename() {
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
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::RenameParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 1,
                    character: 1,
                },
            },
            new_name: "environment".to_string(),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let result = server.rename(params).await.unwrap().unwrap();

        let edits = vec![
            lsp::TextEdit {
                new_text: "environment".to_string(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 1,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 1,
                        character: 3,
                    },
                },
            },
            lsp::TextEdit {
                new_text: "environment".to_string(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 8,
                        character: 34,
                    },
                    end: lsp::Position {
                        line: 8,
                        character: 37,
                    },
                },
            },
        ];
        let mut changes: HashMap<lsp::Url, Vec<lsp::TextEdit>> =
            HashMap::new();
        changes.insert(
            lsp::Url::parse("file:///home/user/file.flux").unwrap(),
            edits,
        );

        let expected = lsp::WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        };

        assert_eq!(expected, result);
    }

    #[test]
    async fn test_references() {
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
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::ReferenceParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 1,
                    character: 1,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: lsp::ReferenceContext {
                // declaration is included whether this is true or false
                include_declaration: true,
            },
        };

        let result =
            server.references(params.clone()).await.unwrap().unwrap();

        let expected = vec![
            lsp::Location {
                uri: params
                    .text_document_position
                    .text_document
                    .uri
                    .clone(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 1,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 1,
                        character: 3,
                    },
                },
            },
            lsp::Location {
                uri: params
                    .text_document_position
                    .text_document
                    .uri
                    .clone(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 8,
                        character: 34,
                    },
                    end: lsp::Position {
                        line: 8,
                        character: 37,
                    },
                },
            },
        ];

        assert_eq!(expected, result);
    }

    #[test]
    async fn test_references_duplicates() {
        let fluxscript = r#"
t = (x) => {
    x = 1
    return x
        // ^
}"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::ReferenceParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: position_of(fluxscript),
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: lsp::ReferenceContext {
                // declaration is included whether this is true or false
                include_declaration: true,
            },
        };

        let result =
            server.references(params.clone()).await.unwrap().unwrap();

        expect![[r#"
            [
              {
                "uri": "file:///home/user/file.flux",
                "range": {
                  "start": {
                    "line": 2,
                    "character": 4
                  },
                  "end": {
                    "line": 2,
                    "character": 5
                  }
                }
              },
              {
                "uri": "file:///home/user/file.flux",
                "range": {
                  "start": {
                    "line": 3,
                    "character": 11
                  },
                  "end": {
                    "line": 3,
                    "character": 12
                  }
                }
              }
            ]"#]]
        .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }

    #[test]
    async fn test_document_highlight() {
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
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::DocumentHighlightParams {
            text_document_position_params:
                lsp::TextDocumentPositionParams {
                    text_document: lsp::TextDocumentIdentifier {
                        uri: lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    },
                    position: lsp::Position {
                        line: 1,
                        character: 1,
                    },
                },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let result = server
            .document_highlight(params.clone())
            .await
            .unwrap()
            .unwrap();

        let expected = vec![
            lsp::DocumentHighlight {
                kind: Some(lsp::DocumentHighlightKind::TEXT),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 1,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 1,
                        character: 3,
                    },
                },
            },
            lsp::DocumentHighlight {
                kind: Some(lsp::DocumentHighlightKind::TEXT),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 8,
                        character: 34,
                    },
                    end: lsp::Position {
                        line: 8,
                        character: 37,
                    },
                },
            },
        ];

        assert_eq!(expected, result);
    }

    fn hover_params(pos: lsp::Position) -> lsp::HoverParams {
        lsp::HoverParams {
            text_document_position_params:
                lsp::TextDocumentPositionParams::new(
                    lsp::TextDocumentIdentifier::new(
                        lsp::Url::parse(
                            "file:///home/user/file.flux",
                        )
                        .unwrap(),
                    ),
                    pos,
                ),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        }
    }

    #[test]
    async fn test_hover() {
        let fluxscript = r#"x = 1
x + 1
"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = hover_params(lsp::Position::new(1, 1));

        let result = server.hover(params).await.unwrap();

        assert_eq!(
            result,
            Some(lsp::Hover {
                contents: lsp::HoverContents::Scalar(
                    lsp::MarkedString::String(
                        "type: int".to_string()
                    )
                ),
                range: None,
            })
        );
    }

    #[test]
    async fn test_hover_binding() {
        let fluxscript = r#"x = "asd"
builtin builtin_ : (v: int) => int
option option_ = 123
1
"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let result = server
            .hover(hover_params(lsp::Position::new(0, 1)))
            .await
            .unwrap();

        assert_eq!(
            result,
            Some(lsp::Hover {
                contents: lsp::HoverContents::Scalar(
                    lsp::MarkedString::String(
                        "type: string".to_string()
                    )
                ),
                range: None,
            })
        );

        let result = server
            .hover(hover_params(lsp::Position::new(1, 12)))
            .await
            .unwrap();

        assert_eq!(
            result,
            Some(lsp::Hover {
                contents: lsp::HoverContents::Scalar(
                    lsp::MarkedString::String(
                        "type: (v:int) => int".to_string()
                    )
                ),
                range: None,
            })
        );

        let result = server
            .hover(hover_params(lsp::Position::new(2, 10)))
            .await
            .unwrap();

        assert_eq!(
            result,
            Some(lsp::Hover {
                contents: lsp::HoverContents::Scalar(
                    lsp::MarkedString::String(
                        "type: int".to_string()
                    )
                ),
                range: None,
            })
        );
    }

    #[test]
    async fn test_hover_argument() {
        let fluxscript = r#"
(x) => x + 1
"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = hover_params(lsp::Position::new(1, 1));

        let result = server.hover(params).await.unwrap();

        assert_eq!(
            result,
            Some(lsp::Hover {
                contents: lsp::HoverContents::Scalar(
                    lsp::MarkedString::String(
                        "type: int".to_string()
                    )
                ),
                range: None,
            })
        );
    }

    #[test]
    async fn test_completion_resolve() {
        let fluxscript = r#"import "strings"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionItem::new_simple(
            "label".to_string(),
            "detail".to_string(),
        );

        let result =
            server.completion_resolve(params.clone()).await.unwrap();

        assert_eq!(params, result);
    }
    #[test]
    async fn test_package_completion() {
        let fluxscript = r#"import "sql"

sql."#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 2,
                    character: 3,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind:
                    lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let expected_labels: Vec<String> = vec!["to", "from"]
            .into_iter()
            .map(|x| x.into())
            .collect::<Vec<String>>();

        match result.clone() {
            lsp::CompletionResponse::List(l) => {
                assert_eq!(
                    expected_labels,
                    l.items
                        .iter()
                        .map(|x| x.label.clone())
                        .collect::<Vec<String>>()
                );
            }
            _ => unreachable!(),
        };
    }

    #[test]
    async fn test_variable_completion() {
        let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

c

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
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 8,
                    character: 1,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let got: BTreeSet<&str> =
            items.iter().map(|i| i.label.as_str()).collect();

        let want: BTreeSet<&str> = vec![
            "buckets",
            "cardinality",
            "chandeMomentumOscillator",
            "columns",
            "contains",
            "contrib/RohanSreerama5/naiveBayesClassifier",
            "contrib/anaisdg/anomalydetection",
            "contrib/bonitoo-io/servicenow",
            "contrib/bonitoo-io/tickscript",
            "contrib/bonitoo-io/victorops",
            "contrib/chobbs/discord",
            "count",
            "cov",
            "covariance",
            "csv",
            "cumulativeSum",
            "dict",
            "difference",
            "distinct",
            "duplicate",
            "experimental/csv",
            "experimental/record",
            "findColumn",
            "findRecord",
            "getColumn",
            "getRecord",
            "highestCurrent",
            "hourSelection",
            "increase",
            "influxdata/influxdb/schema",
            "influxdata/influxdb/secrets",
            "internal/location",
            "logarithmicBins",
            "lowestCurrent",
            "reduce",
            "slack",
            "socket",
            "stateCount",
            "stateTracking",
            "testing/expect",
            "truncateTimeColumn",
        ]
        .drain(..)
        .collect();

        assert_eq!(
            want,
            got,
            "\nextra:\n {:?}\n missing:\n {:?}\n",
            got.difference(&want),
            want.difference(&got)
        );
    }

    // TODO: sean (10 Aug 2021) - This test fails unless the line reading
    // `ab = 10` in the flux script is commented out. The error is valid,
    // but the lsp should be able to turn it into a diagnostic notification
    // and continue to provide completion suggestions.
    //
    // An issue has been created for this:
    // https://github.com/influxdata/flux-lsp/issues/290
    #[test]
    async fn test_option_object_members_completion() {
        let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

option task = {
  name: "foo",        // Name is required.
  every: 1h,          // Task should be run at this interval.
  delay: 10m,         // Delay scheduling this task by this duration.
  cron: "0 2 * * *",  // Cron is a more sophisticated way to schedule. 'every' and 'cron' are mutually exclusive.
  retry: 5,           // Number of times to retry a failed query.
}

task.

// ab = 10
"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 16,
                    character: 5,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind:
                    lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let labels: Vec<&str> =
            items.iter().map(|item| item.label.as_str()).collect();

        let expected = vec![
            "name (self)",
            "every (self)",
            "delay (self)",
            "cron (self)",
            "retry (self)",
        ];

        assert_eq!(expected, labels);
    }

    #[test]
    async fn test_option_function_completion() {
        let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

option now = () => 2020-02-20T23:00:00Z

n

ab = 10
"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 10,
                    character: 1,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let got: BTreeSet<&str> =
            items.iter().map(|i| i.label.as_str()).collect();

        let want: BTreeSet<&str> = vec![
            "aggregateWindow",
            "cardinality",
            "chandeMomentumOscillator",
            "columns",
            "contains",
            "contrib/RohanSreerama5/naiveBayesClassifier",
            "contrib/anaisdg/anomalydetection",
            "contrib/bonitoo-io/zenoss",
            "contrib/bonitoo-io/servicenow",
            "contrib/jsternberg/influxdb",
            "contrib/rhajek/bigpanda",
            "contrib/sranka/opsgenie",
            "contrib/sranka/sensu",
            "contrib/tomhollingworth/events",
            "count",
            "covariance",
            "difference",
            "distinct",
            "duration",
            "experimental",
            "experimental/influxdb",
            "experimental/json",
            "exponentialMovingAverage",
            "findColumn",
            "findRecord",
            "generate",
            "getColumn",
            "highestCurrent",
            "histogramQuantile",
            "holtWinters",
            "hourSelection",
            "increase",
            "inf (prelude)",
            "influxdata/influxdb",
            "influxdata/influxdb/monitor",
            "int",
            "integral",
            "internal/boolean",
            "internal/gen",
            "internal/influxql",
            "internal/location",
            "interpolate",
            "join",
            "json",
            "kaufmansAMA",
            "kaufmansER",
            "length",
            "linearBins",
            "logarithmicBins",
            "lowestCurrent",
            "lowestMin",
            "mean",
            "median",
            "min",
            "movingAverage",
            "now",
            "pearsonr",
            "planner",
            "quantile",
            "range",
            "relativeStrengthIndex",
            "rename",
            "runtime",
            "stateCount",
            "stateDuration",
            "stateTracking",
            "string",
            "strings",
            "tableFind",
            "testing",
            "timedMovingAverage",
            "timezone",
            "toInt",
            "toString",
            "toUInt",
            "tripleExponentialDerivative",
            "truncateTimeColumn",
            "uint",
            "union",
            "unique",
            "universe",
            "window",
        ]
        .drain(..)
        .collect();

        assert_eq!(
            want,
            got,
            "\nextra:\n {:?}\n missing:\n {:?}\n",
            got.difference(&want),
            want.difference(&got)
        );
    }

    #[test]
    async fn test_object_param_completion() {
        let fluxscript = r#"obj = {
    func: (name, age) => name + age
}

obj.func(
        "#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 4,
                    character: 8,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind:
                    lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some("(".to_string()),
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let labels: Vec<&str> =
            items.iter().map(|item| item.label.as_str()).collect();

        let expected = vec!["name", "age"];

        assert_eq!(expected, labels);
    }

    #[test]
    async fn test_param_completion() {
        let fluxscript = r#"import "csv"

csv.from(
        "#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 2,
                    character: 8,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind:
                    lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some("(".to_string()),
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let labels: Vec<&str> =
            items.iter().map(|item| item.label.as_str()).collect();

        let expected = vec!["csv", "file", "mode", "url"];

        assert_eq!(expected, labels);
    }

    #[test]
    async fn test_options_completion() {
        let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

option task = {
  name: "foo",        // Name is required.
  every: 1h,          // Task should be run at this interval.
  delay: 10m,         // Delay scheduling this task by this duration.
  cron: "0 2 * * *",  // Cron is a more sophisticated way to schedule. 'every' and 'cron' are mutually exclusive.
  retry: 5,           // Number of times to retry a failed query.
}

newNow = t

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d )
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
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 16,
                    character: 10,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let got: BTreeSet<&str> =
            items.iter().map(|i| i.label.as_str()).collect();

        let want: BTreeSet<&str> = vec![
            "aggregateWindow",
            "bottom",
            "buckets",
            "bytes",
            "cardinality",
            "chandeMomentumOscillator",
            "contains",
            "contrib/anaisdg/anomalydetection",
            "contrib/anaisdg/statsmodels",
            "contrib/bonitoo-io/alerta",
            "contrib/bonitoo-io/tickscript",
            "contrib/bonitoo-io/victorops",
            "contrib/jsternberg/aggregate",
            "contrib/jsternberg/math",
            "contrib/sranka/teams",
            "contrib/sranka/telegram",
            "contrib/sranka/webexteams",
            "contrib/tomhollingworth/events",
            "count",
            "cumulativeSum",
            "date",
            "derivative",
            "dict",
            "distinct",
            "duplicate",
            "duration",
            "experimental",
            "experimental/aggregate",
            "experimental/bigtable",
            "experimental/bitwise",
            "experimental/http",
            "experimental/mqtt",
            "experimental/prometheus",
            "experimental/table",
            "exponentialMovingAverage",
            "filter",
            "first",
            "float",
            "generate",
            "getColumn",
            "getRecord",
            "highestAverage",
            "highestCurrent",
            "highestMax",
            "histogram",
            "histogramQuantile",
            "holtWinters",
            "hourSelection",
            "http",
            "influxdata/influxdb/monitor",
            "influxdata/influxdb/secrets",
            "influxdata/influxdb/tasks",
            "int",
            "integral",
            "internal/testutil",
            "internal/location",
            "interpolate",
            "last",
            "length",
            "limit",
            "logarithmicBins",
            "lowestAverage",
            "lowestCurrent",
            "lowestMin",
            "math",
            "pagerduty",
            "pivot",
            "pushbullet",
            "quantile",
            "relativeStrengthIndex",
            "runtime",
            "sampledata",
            "set",
            "socket",
            "sort",
            "stateCount",
            "stateDuration",
            "stateTracking",
            "stddev",
            "string",
            "strings",
            "system",
            "tableFind",
            "tail",
            "testing",
            "testing/expect",
            "time",
            "timeShift",
            "timeWeightedAvg",
            "timedMovingAverage",
            "timezone",
            "to",
            "toBool",
            "toFloat",
            "toInt",
            "toString",
            "toTime",
            "toUInt",
            "today",
            "top",
            "tripleEMA",
            "tripleExponentialDerivative",
            "true (prelude)",
            "truncateTimeColumn",
            "types",
            "uint",
        ]
        .drain(..)
        .collect();

        assert_eq!(
            want,
            got,
            "\nextra:\n {:?}\n missing:\n {:?}\n",
            got.difference(&want),
            want.difference(&got)
        );
    }

    #[test]
    async fn test_signature_help_invalid() {
        let fluxscript = r#"bork |>"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

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

        let result = server.signature_help(params).await;
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    async fn test_folding_range_invalid() {
        let fluxscript = r#"bork |>"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

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

        let result = server.folding_range(params).await;
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    async fn test_document_symbol_invalid() {
        let fluxscript = r#"bork |>"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

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
        let result = server.document_symbol(params).await;

        assert!(matches!(result, Ok(None)));
    }

    #[test]
    async fn test_goto_definition_invalid() {
        let fluxscript = r#"bork |>"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

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

        let result = server.goto_definition(params).await;
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    async fn test_rename_invalid() {
        let fluxscript = r#"bork |>"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::ReferenceParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 1,
                    character: 1,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: lsp::ReferenceContext {
                // declaration is included whether this is true or false
                include_declaration: true,
            },
        };

        let result = server.references(params.clone()).await;

        assert!(matches!(result, Ok(None)));
    }

    #[test]
    async fn test_package_completion_when_it_is_imported() {
        let fluxscript = r#"import "sql"

sql"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::CompletionParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 2,
                    character: 2,
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        // We should not try to insert the `sql` import again
        expect_test::expect![[r#"
            {
              "isIncomplete": false,
              "items": [
                {
                  "label": "sql",
                  "kind": 9,
                  "detail": "Package",
                  "documentation": "sql",
                  "sortText": "sql",
                  "filterText": "sql",
                  "insertText": "sql",
                  "insertTextFormat": 1,
                  "additionalTextEdits": []
                }
              ]
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }
}
