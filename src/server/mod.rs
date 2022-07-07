mod commands;
mod store;
mod transform;
mod types;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use flux::ast::walk::Node as AstNode;
use flux::ast::Expression as AstExpression;
use flux::semantic::nodes::{
    ErrorKind as SemanticNodeErrorKind, Package as SemanticPackage,
};
use flux::semantic::types::{BuiltinType, CollectionType, MonoType};
use flux::semantic::{walk, ErrorKind};
use lspower::{
    jsonrpc::Result as RpcResult, lsp, Client, LanguageServer,
};

use crate::{completion, stdlib, visitors::semantic};

use self::commands::{
    InjectFieldFilterParams, InjectMeasurementFilterParams,
    InjectTagFilterParams, InjectTagValueFilterParams,
    LspServerCommand,
};
use self::types::LspError;

const VERSION: &str = env!("CARGO_PKG_VERSION");

type Diagnostic =
    fn(&SemanticPackage) -> Vec<(Option<String>, lsp::Diagnostic)>;

/// Convert a flux::semantic::walk::Node to a lsp::Location
/// https://microsoft.github.io/language-server-protocol/specification#location
fn node_to_location(
    node: &flux::semantic::walk::Node,
    uri: lsp::Url,
) -> lsp::Location {
    lsp::Location {
        uri,
        range: node.loc().clone().into(),
    }
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

fn find_references<'a>(
    uri: &lsp::Url,
    node: Option<flux::semantic::walk::Node<'a>>,
    path: Vec<flux::semantic::walk::Node<'a>>,
) -> Vec<lsp::Location> {
    if let Some(node) = node {
        let name = match node {
            walk::Node::Identifier(ident) => &ident.name,
            walk::Node::IdentifierExpr(ident) => &ident.name,
            _ => return Vec::new(),
        };

        let mut path_iter = path.iter().rev();
        let scope: walk::Node =
            match path_iter.find_map(|n| match n {
                walk::Node::FunctionExpr(f)
                    if f.params
                        .iter()
                        .any(|param| &param.key.name == name) =>
                {
                    Some(n)
                }
                walk::Node::Package(_) | walk::Node::File(_) => {
                    let mut visitor =
                        semantic::DefinitionFinderVisitor::new(
                            name.clone(),
                        );
                    walk::walk(&mut visitor, *n);

                    if visitor.node.is_some() {
                        Some(n)
                    } else {
                        None
                    }
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
            .map(|node| node_to_location(node, uri.clone()))
            .collect();
        locations
    } else {
        Vec::new()
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
            x.signatures().into_iter().map(|signature| {
                lsp::SignatureInformation {
                    label: signature.create_signature(),
                    parameters: Some(signature.create_parameters()),
                    documentation: None,
                    active_parameter: None,
                }
            })
        })
        .fold(vec![], |mut acc, x| {
            acc.extend(x);
            acc
        })
}

#[derive(Default)]
struct LspServerState {
    buckets: Vec<String>,
}

impl LspServerState {
    // XXX: rockstar (21 Jun 2022) - This `allow` pragma is temporary, until we can add
    // bucket completion, which is blocked on the completion refactor.
    #[allow(dead_code)]
    pub fn buckets(&self) -> &Vec<String> {
        &self.buckets
    }

    pub fn set_buckets(&mut self, buckets: Vec<String>) {
        self.buckets = buckets;
    }
}

pub struct LspServer {
    client: Arc<Mutex<Option<Client>>>,
    diagnostics: Vec<Diagnostic>,
    store: store::Store,
    state: Mutex<LspServerState>,
}

impl LspServer {
    pub fn new(client: Option<Client>) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            diagnostics: vec![
                super::diagnostics::contrib_lint,
                super::diagnostics::experimental_lint,
                super::diagnostics::no_influxdb_identifiers,
            ],
            store: store::Store::default(),
            state: Mutex::new(LspServerState::default()),
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
        match self.store.get(key) {
            Ok(contents) => Ok(contents),
            Err(err) => Err(err.into()),
        }
    }

    /// Publish any diagnostics to the client
    async fn publish_diagnostics(&self, key: &lsp::Url) {
        // If we have a client back to the editor report any diagnostics found in the document
        if let Some(client) = &self.get_client() {
            for (key, diagnostics) in
                self.compute_diagnostics(key).into_iter()
            {
                client
                    .publish_diagnostics(key, diagnostics, None)
                    .await;
            }
        }
    }

    /// Compute diagnostics for a package
    ///
    /// This function will compute all diagnostics for the same package simultaneously. This
    /// includes files that don't have any diagnostic messages (an empty list is generated),
    /// as this is the way the server will signal that previous diagnostic messages have cleared.
    fn compute_diagnostics(
        &self,
        key: &lsp::Url,
    ) -> HashMap<lsp::Url, Vec<lsp::Diagnostic>> {
        let mut diagnostic_map: HashMap<
            lsp::Url,
            Vec<lsp::Diagnostic>,
        > = self
            .store
            .get_package_urls(key)
            .into_iter()
            .map(|url| (url, Vec::new()))
            .collect();

        let diagnostics: Vec<(Option<String>, lsp::Diagnostic)> =
            match self.store.get_package_errors(key) {
                None => {
                    // If there are no semantic package errors, we can check for other
                    // diagnostics.
                    //
                    // Note: it is important, if no diagnostics exist, that we return an empty
                    // diagnostic list, as that will signal to the client that the diagnostics
                    // have been cleared.
                    if let Ok(package) =
                        self.store.get_semantic_package(key)
                    {
                        self
                        .diagnostics
                        .iter()
                        .flat_map(|func| func(&package))
                        .collect::<Vec<(Option<String>, lsp::Diagnostic)>>()
                    } else {
                        vec![]
                    }
                }
                Some(errors) => {
                    errors
                        .diagnostics
                        .errors
                        .iter()
                        .filter(|error| {
                            // We will never have two files with the same name in a package, so we can
                            // key off filename to determine whether the error exists in this file or
                            // elsewhere in the package.
                            if let Some(file) = &error.location.file {
                                if let Some(segments) =
                                    key.path_segments()
                                {
                                    if let Some(filename) =
                                        segments.last()
                                    {
                                        return file == filename;
                                    }
                                }
                            }
                            false
                        })
                        .map(|e| {
                            (e.location.file.clone(), lsp::Diagnostic {
                    range: e.location.clone().into(),
                    severity: Some(lsp::DiagnosticSeverity::ERROR),
                    source: Some("flux".to_string()),
                    message: e.error.to_string(),
                    ..lsp::Diagnostic::default()
                })
                        })
                        .collect()
                }
            };
        diagnostics.into_iter().for_each(|(filename, diagnostic)| {
            // XXX: rockstar (5 June 2022) - Can this _ever_ be None? Is a blind unwrap safe?
            if let Some(filename) = filename {
                diagnostic_map
                    .iter_mut()
                    .filter(|(url, _)| {
                        url.to_string().ends_with(&filename)
                    })
                    .for_each(|(_, diagnostics)| {
                        diagnostics.push(diagnostic.clone())
                    });
            }
        });

        diagnostic_map
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
                code_action_provider: Some(lsp::CodeActionProviderCapability::Simple(true)),
                code_lens_provider: None,
                color_provider: None,
                completion_provider: Some(lsp::CompletionOptions {
                    resolve_provider: None,
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
                execute_command_provider: Some(lsp::ExecuteCommandOptions {
                    commands: vec![LspServerCommand::InjectTagFilter.into(), LspServerCommand::InjectTagValueFilter.into(), LspServerCommand::InjectFieldFilter.into(), LspServerCommand::InjectMeasurementFilter.into()],
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: None,
                    }
                }),
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
                references_provider: Some(lsp::OneOf::Left(true)),
                rename_provider: Some(lsp::OneOf::Left(true)),
                selection_range_provider: None,
                semantic_tokens_provider: Some(lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(lsp::SemanticTokensOptions{
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: None
                    },
                    legend: lsp::SemanticTokensLegend {
                        token_types: crate::visitors::ast::SemanticToken::LSP_MAPPING.to_owned(),
                        token_modifiers: vec![],
                    },
                    range: None,
                    full: None,
                })),
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
                version: Some(VERSION.into()),
            }),
        })
    }

    async fn shutdown(&self) -> RpcResult<()> {
        // XXX: rockstar (19 May 2022) - This chunk of code will no longer be needed,
        // when tower-lsp is added again.
        let mut client = match self.client.lock() {
            Ok(client) => client,
            Err(err) => {
                return Err(LspError::InternalError(format!(
                    "{}",
                    err
                ))
                .into())
            }
        };
        *client = None;

        Ok(())
    }

    async fn did_open(
        &self,
        params: lsp::DidOpenTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;
        let value = params.text_document.text;
        self.store.put(&key, &value);

        self.publish_diagnostics(&key).await;
    }

    async fn did_change(
        &self,
        params: lsp::DidChangeTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;

        match self.store.get(&key) {
            Ok(value) => {
                let mut contents = Cow::Borrowed(&value);
                for change in params.content_changes {
                    contents = Cow::Owned(
                        if let Some(range) = change.range {
                            replace_string_in_range(
                                contents.into_owned(),
                                range,
                                change.text,
                            )
                        } else {
                            change.text
                        },
                    );
                }
                let new_contents = contents.into_owned();
                self.store.put(&key, &new_contents.clone());
                self.publish_diagnostics(&key).await;
            }
            Err(err) => log::error!(
                "Could not update key: {}\n{:?}",
                key,
                err
            ),
        }
    }

    async fn did_save(
        &self,
        params: lsp::DidSaveTextDocumentParams,
    ) -> () {
        if let Some(text) = params.text {
            let key = params.text_document.uri;
            self.store.put(&key, &text);
            self.publish_diagnostics(&key).await;
        }
    }

    async fn did_close(
        &self,
        params: lsp::DidCloseTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;
        self.store.remove(&key);
    }

    async fn did_change_configuration(
        &self,
        params: lsp::DidChangeConfigurationParams,
    ) -> () {
        if let serde_json::value::Value::Object(map) = params.settings
        {
            if let Some(settings) = map.get("settings") {
                if let Some(serde_json::value::Value::Array(
                    buckets,
                )) = settings.get("buckets")
                {
                    match self.state.lock() {
                        Ok(mut state) => {
                            state.set_buckets(
                                buckets
                                    .iter()
                                    .filter(|bucket| {
                                        bucket.is_string()
                                    })
                                    .map(|bucket| {
                                        #[allow(clippy::unwrap_used)]
                                        String::from(
                                            bucket.as_str().unwrap(),
                                        )
                                    })
                                    .collect::<Vec<String>>(),
                            );
                        }
                        Err(err) => log::error!("{}", err),
                    }
                }
            }
        }
    }

    async fn signature_help(
        &self,
        params: lsp::SignatureHelpParams,
    ) -> RpcResult<Option<lsp::SignatureHelp>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };

        let mut signatures = vec![];
        let mut visitor = semantic::NodeFinderVisitor::new(
            params.text_document_position_params.position,
        );
        flux::semantic::walk::walk(
            &mut visitor,
            walk::Node::Package(&pkg),
        );

        if let Some(node) = visitor.node {
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
                } else {
                    log::debug!("signature_help on non-member and non-identifier");
                }
            } else {
                log::debug!("signature_help on non-call expression");
            }
        }

        let response = if signatures.is_empty() {
            None
        } else {
            Some(lsp::SignatureHelp {
                signatures,
                active_signature: None,
                active_parameter: None,
            })
        };
        Ok(response)
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
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
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

        Ok(if results.is_empty() {
            None
        } else {
            Some(results)
        })
    }

    async fn document_symbol(
        &self,
        params: lsp::DocumentSymbolParams,
    ) -> RpcResult<Option<lsp::DocumentSymbolResponse>> {
        let key = params.text_document.uri;
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
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

        let response = if symbols.is_empty() {
            None
        } else {
            Some(lsp::DocumentSymbolResponse::Flat(symbols))
        };

        Ok(response)
    }

    async fn goto_definition(
        &self,
        params: lsp::GotoDefinitionParams,
    ) -> RpcResult<Option<lsp::GotoDefinitionResponse>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };

        let pkg_node = walk::Node::Package(&pkg);
        let mut visitor = semantic::NodeFinderVisitor::new(
            params.text_document_position_params.position,
        );

        flux::semantic::walk::walk(&mut visitor, pkg_node);

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
                let location = node_to_location(&node, key);
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
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };

        let mut visitor = semantic::NodeFinderVisitor::new(
            params.text_document_position.position,
        );
        flux::semantic::walk::walk(
            &mut visitor,
            walk::Node::Package(&pkg),
        );

        let locations =
            find_references(&key, visitor.node, visitor.path);
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
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };

        let mut visitor = semantic::NodeFinderVisitor::new(
            params.text_document_position_params.position,
        );
        flux::semantic::walk::walk(
            &mut visitor,
            walk::Node::Package(&pkg),
        );

        let refs = find_references(&key, visitor.node, visitor.path);
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
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };

        let mut visitor = semantic::NodeFinderVisitor::new(
            params.text_document_position.position,
        );
        flux::semantic::walk::walk(
            &mut visitor,
            walk::Node::Package(&pkg),
        );

        let references =
            find_references(&key, visitor.node, visitor.path);
        Ok(if references.is_empty() {
            None
        } else {
            Some(references)
        })
    }

    async fn hover(
        &self,
        params: lsp::HoverParams,
    ) -> RpcResult<Option<lsp::Hover>> {
        let key =
            params.text_document_position_params.text_document.uri;
        let pkg = match self.store.get_semantic_package(&key) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };

        let mut visitor = semantic::NodeFinderVisitor::new(
            params.text_document_position_params.position,
        );

        flux::semantic::walk::walk(
            &mut visitor,
            walk::Node::Package(&pkg),
        );

        if let Some(node) = visitor.node {
            let path = &visitor.path;
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

                        // The type of an property identifier is the type of the value
                        walk::Node::Property(property) => {
                            Some(property.value.type_of())
                        }

                        // The type Function parameters can be derived from the function type
                        // stored in the function expression
                        walk::Node::FunctionParameter(_) => {
                            let func = path.get(path.len() - 3)?;
                            match func {
                                walk::Node::FunctionExpr(func) => {
                                    func.typ
                                        .parameter(
                                            ident.name.as_str(),
                                        )
                                        .cloned()
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
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

    async fn completion(
        &self,
        params: lsp::CompletionParams,
    ) -> RpcResult<Option<lsp::CompletionResponse>> {
        let ast_pkg = match self.store.get_ast_package(
            &params.text_document_position.text_document.uri,
        ) {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };
        let sem_pkg = match self.store.get_semantic_package(
            &params.text_document_position.text_document.uri,
        ) {
            Ok(pkg) => pkg,
            Err(err) => {
                return Err(err.into());
            }
        };

        let position = params.text_document_position.position.clone();
        let walker = flux::ast::walk::Node::Package(&ast_pkg);
        let mut visitor =
            crate::visitors::ast::NodeFinderVisitor::new(position);

        flux::ast::walk::walk(&mut visitor, walker);

        let items = match visitor.node {
            Some(walk_node) => match walk_node.node {
                AstNode::CallExpr(call) => {
                    completion::complete_call_expr(
                        &params, &sem_pkg, call,
                    )
                }
                AstNode::Identifier(identifier) => {
                    // XXX: rockstar (6 Jul 2022) - This is helping to complete packages that
                    // have never been imported. That's probably not a great pattern.
                    let stdlib_completions: Vec<lsp::CompletionItem> =
                        if let Some(env) = flux::imports() {
                            env.iter().filter(|(key, _val)| {
                            if let Some(package_name) = crate::shared::get_package_name(key) {
                                completion::fuzzy_match(package_name, &identifier.name)
                            } else {
                                false
                            }
                        }).map(|(key, _val)| {
                            #[allow(clippy::unwrap_used)]
                            let package_name = crate::shared::get_package_name(key).unwrap();
                            lsp::CompletionItem {
                                label: key.clone(),
                                detail: Some("Package".into()),
                                documentation: Some(lsp::Documentation::String(
                                    key.clone(),
                                )),
                                filter_text: Some(package_name.into()),
                                insert_text: Some(key.clone()),
                                insert_text_format: Some(lsp::InsertTextFormat::PLAIN_TEXT),
                                kind: Some(lsp::CompletionItemKind::MODULE),
                                sort_text: Some(key.clone()),
                                ..lsp::CompletionItem::default()
                            }
                        }).collect()
                        } else {
                            vec![]
                        };

                    let builtin_completions: Vec<
                        lsp::CompletionItem,
                    > = if let Some(env) = flux::prelude() {
                        env.iter().filter(|(key, val)| {
                            // Don't allow users to "discover" private-ish functionality.
                            // Filter out irrelevent items that won't match.
                            // Only pass expressions that have completion support.
                            !key.starts_with('_') && completion::fuzzy_match(key, &identifier.name) &&
                            match &val.expr {
                                MonoType::Fun(_) | MonoType::Builtin(_) => true,
                                MonoType::Collection(collection) => collection.collection == CollectionType::Array,
                                _ => false
                            }
                        }).map(|(key, val)| {
                            match &val.expr {
                                MonoType::Fun(function) => {
                                    lsp::CompletionItem {
                                        label: key.into(),
                                        detail: Some(stdlib::create_function_signature(function)),
                                        filter_text: Some(key.into()),
                                        insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
                                        kind: Some(lsp::CompletionItemKind::FUNCTION),
                                        sort_text: Some(key.into()),
                                        ..lsp::CompletionItem::default()
                                    }
                                }
                                MonoType::Collection(_collection) => {
                                    // name: key
                                    // package: PRELUDE_PACKAGE
                                    // package_name: None,
                                    // var_type VarType::Array
                                    lsp::CompletionItem {
                                        label: format!("{} ({})", key, "prelude"),
                                        detail: Some("Array".into()),
                                        documentation: Some(lsp::Documentation::String(format!("from prelude"))),
                                        filter_text: Some(key.into()),
                                        insert_text: Some(key.into()),
                                        insert_text_format: Some(
                                            lsp::InsertTextFormat::PLAIN_TEXT
                                        ),
                                        kind: Some(lsp::CompletionItemKind::VARIABLE),
                                        sort_text: Some(format!("{} prelude", key)),
                                        ..lsp::CompletionItem::default()
                                    }
                                }
                                MonoType::Builtin(builtin) => {
                                    // name: key
                                    // package: PRELUDE_PACKAGE
                                    // package_name: None,
                                    // var_type VarType::from(*b)
                                    lsp::CompletionItem {
                                        label: format!("{} ({})", key, "prelude"),
                                        detail: Some(match *builtin {
                                            BuiltinType::String => "String".into(),
                                            BuiltinType::Int => "Integer".into(),
                                            BuiltinType::Float => "Float".into(),
                                            BuiltinType::Bool => "Boolean".into(),
                                            BuiltinType::Bytes => "Bytes".into(),
                                            BuiltinType::Duration => "Duration".into(),
                                            BuiltinType::Uint => "Uint".into(),
                                            BuiltinType::Regexp => "Regular Expression".into(),
                                            BuiltinType::Time => "Time".into(),
                                        }),
                                        documentation: Some(lsp::Documentation::String(format!("from prelude"))),
                                        filter_text: Some(key.into()),
                                        insert_text: Some(key.into()),
                                        insert_text_format: Some(
                                            lsp::InsertTextFormat::PLAIN_TEXT
                                        ),
                                        kind: Some(lsp::CompletionItemKind::VARIABLE),
                                        sort_text: Some(format!("{} prelude", key)),
                                        ..lsp::CompletionItem::default()
                                    }
                                }
                                _ => unreachable!("Previous filter on expression value failed. Got: {}", val.expr)
                            }
                        }).collect()
                    } else {
                        vec![]
                    };

                    vec![stdlib_completions, builtin_completions]
                        .into_iter()
                        .flatten()
                        .collect()
                }
                AstNode::MemberExpr(member) => {
                    match &member.object {
                        AstExpression::Identifier(identifier) => {
                            // XXX: rockstar (6 Jul 2022) - This is the last holdout from the previous
                            // completion code. There is a bit of indirection/cruft here that can be cleaned
                            // up when recursive support for member expressions is implemented.
                            let mut list: Vec<
                                Box<dyn completion::Completable>,
                            > = vec![];
                            if let Some(env) = flux::imports() {
                                if let Some(import) =
                                    completion::get_imports(&sem_pkg)
                                        .iter()
                                        .find(|x| {
                                            x.alias == identifier.name
                                        })
                                {
                                    for (key, val) in env.iter() {
                                        if *key == import.path {
                                            completion::walk_package(
                                                key,
                                                &mut list,
                                                &val.typ().expr,
                                            );
                                        }
                                    }
                                } else {
                                    for (key, val) in env.iter() {
                                        if let Some(package_name) =
                                            crate::shared::get_package_name(key)
                                        {
                                            if package_name == identifier.name {
                                                completion::walk_package(
                                                    key,
                                                    &mut list,
                                                    &val.typ().expr,
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            let walker =
                                flux::semantic::walk::Node::Package(
                                    &sem_pkg,
                                );
                            let mut visitor =
                                completion::CompletableObjectFinderVisitor::new(
                                    &identifier.name,
                                );
                            flux::semantic::walk::walk(
                                &mut visitor,
                                walker,
                            );

                            let imports =
                                completion::get_imports(&sem_pkg);
                            vec![
                                visitor.completables.iter().map(|completable| completable.completion_item(&imports)).collect::<Vec<lsp::CompletionItem>>(),
                                list.iter().map(|completable| completable.completion_item(&imports)).collect(),
                            ].into_iter().flatten().collect()
                        }
                        _ => return Ok(None),
                    }
                }
                AstNode::ObjectExpr(_) => {
                    let parent = walk_node
                        .parent
                        .as_ref()
                        .map(|parent| &parent.node);
                    match parent {
                        Some(AstNode::CallExpr(call)) => {
                            completion::complete_call_expr(
                                &params, &sem_pkg, call,
                            )
                        }
                        Some(_) => vec![],
                        None => vec![],
                    }
                }
                AstNode::StringLit(_) => {
                    let parent = walk_node
                        .parent
                        .as_ref()
                        .map(|parent| &parent.node);
                    match parent {
                        Some(AstNode::ImportDeclaration(_)) => {
                            let infos: Vec<(String, String)> =
                                if let Some(env) = flux::imports() {
                                    env.iter().filter(|(path, _val)| {
                                    crate::shared::get_package_name(path).is_some()
                                }).map(|(path, _val)| {
                                    #[allow(clippy::expect_used)]
                                    (crate::shared::get_package_name(path).expect("Previous filter failed.").into(), path.clone())
                                }).collect()
                                } else {
                                    vec![]
                                };
                            let imports =
                                completion::get_imports(&sem_pkg);

                            infos.into_iter().filter(|(name, _path)| {
                                !&imports.iter().any(|x| &x.path == name)
                            }).map(|(_name, path)| {
                                let trigger = if let Some(context) = & params.context {
                                    context.trigger_character.as_deref()
                                } else {
                                    None
                                };
                                let insert_text = if trigger == Some("\"") {
                                    path.as_str().to_string()
                                } else {
                                    format!(r#""{}""#, path.as_str())
                                };
                                lsp::CompletionItem {
                                    label: insert_text.clone(),
                                    insert_text: Some(insert_text),
                                    insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
                                    kind: Some(lsp::CompletionItemKind::VALUE),
                                    ..lsp::CompletionItem::default()
                                }
                            }).collect()
                        }
                        // This is where bucket/measurement/field/tag completion will occur.
                        Some(_) => vec![],
                        None => vec![],
                    }
                }
                _ => return Ok(None),
            },
            None => return Ok(None),
        };
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lsp::CompletionResponse::List(
                lsp::CompletionList {
                    // XXX: rockstar (5 Jul 2022) - This should probably always be incomplete, so
                    // we don't leave off to the client to try and figure out what completions to use.
                    is_incomplete: false,
                    items,
                },
            )))
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: lsp::SemanticTokensParams,
    ) -> RpcResult<Option<lsp::SemanticTokensResult>> {
        let pkg = match self
            .store
            .get_ast_package(&params.text_document.uri)
        {
            Ok(pkg) => pkg,
            Err(err) => return Err(err.into()),
        };
        let root_node = flux::ast::walk::Node::File(&pkg.files[0]);

        let mut visitor =
            crate::visitors::ast::SemanticTokenVisitor::default();

        flux::ast::walk::walk(&mut visitor, root_node);

        Ok(Some(lsp::SemanticTokensResult::Tokens(
            lsp::SemanticTokens {
                result_id: None,
                data: visitor.tokens.clone(),
            },
        )))
    }

    // The use of unwrap/expect here is intentional, and should only occur with prior
    // checks in place. If we were to use nested matchers, it makes the code difficult
    // to reason about.
    #[allow(clippy::expect_used)]
    async fn code_action(
        &self,
        params: lsp::CodeActionParams,
    ) -> RpcResult<Option<lsp::CodeActionResponse>> {
        // Our code actions should all be connected with a diagnostic. The
        // client user experience can vary when not directly connected to
        // a diagnostic, which is sorta the client's fault, but we also
        // don't have a need for trying to support any other flows.
        if params.context.diagnostics.is_empty() {
            return Ok(None);
        }

        let errors = match self
            .store
            .get_package_errors(&params.text_document.uri)
        {
            Some(errors) => errors,
            None => return Ok(None),
        };

        let relevant: Vec<&flux::semantic::Error> = errors
            .diagnostics
            .errors
            .iter()
            .filter(|error| {
                crate::lsp::ranges_overlap(
                    &params.range,
                    &error.location.clone().into(),
                )
            })
            .collect();
        if relevant.is_empty() {
            return Ok(None);
        }

        let pkg = match self
            .store
            .get_semantic_package(&params.text_document.uri)
        {
            Ok(pkg) => pkg,
            Err(err) => unreachable!("{:?}", err),
        };
        let mut visitor =
            semantic::PackageNodeFinderVisitor::default();
        let walker = walk::Node::Package(&pkg);
        walk::walk(&mut visitor, walker);

        let import_position = match visitor.location {
            Some(location) => lsp::Position {
                line: location.start.line + 1,
                character: 0,
            },
            None => lsp::Position::default(),
        };

        let actions: Vec<lsp::CodeActionOrCommand> = relevant.iter().map(|error| {
            if let ErrorKind::Inference(kind) = &error.error {
                match kind {
                    SemanticNodeErrorKind::UndefinedIdentifier(identifier) => {
                        // When encountering undefined identifiers, check to see if they match a corresponding
                        // package available for import.
                        let imports = flux::imports()?;
                        let potential_imports: Vec<&String> = imports.iter().filter(|x| match crate::shared::get_package_name(x.0) {
                            Some(name) => name == identifier,
                            None => false,
                        }).map(|x| x.0 ).collect();
                        if potential_imports.is_empty() {
                            return None;
                        }

                        let inner_actions: Vec<lsp::CodeActionOrCommand> = potential_imports.iter().map(|package_name| {
                            lsp::CodeAction {
                                title: format!("Import `{}`", package_name),
                                kind: Some(lsp::CodeActionKind::QUICKFIX),
                                diagnostics: None,
                                edit: Some(lsp::WorkspaceEdit {
                                    changes: Some(HashMap::from([
                                        (params.text_document.uri.clone(), vec![
                                            lsp::TextEdit {
                                                range: lsp::Range {
                                                    start: import_position,
                                                    end: import_position,
                                                },
                                                new_text: format!("import \"{}\"\n", package_name),
                                            }
                                        ])
                                    ])),
                                    document_changes: None,
                                    change_annotations: None,
                                }),
                                command: None,
                                is_preferred: Some(true),
                                disabled: None,
                                data: None,
                            }.into()
                        }).collect();
                        return Some(inner_actions);
                    },
                    _ => return None,
                }
            }
            None
        }).filter(|action| action.is_some()).flat_map(|action| {
            action.expect("Previous .filter() call failed.")
        }).collect();

        return Ok(Some(actions));
    }

    async fn execute_command(
        &self,
        params: lsp::ExecuteCommandParams,
    ) -> RpcResult<Option<serde_json::Value>> {
        if params.arguments.len() > 1
            || (params.arguments.len() == 1
                && !params.arguments[0].is_object())
        {
            // We want, at most, a single argument, which is an object itself. This means that
            // positional arguments are not supported. We only want kwargs. Some commands will
            // take no arguments.
            return Err(
                LspError::InvalidArguments(params.arguments).into()
            );
        }
        match LspServerCommand::try_from(params.command.clone()) {
            Ok(LspServerCommand::InjectTagFilter) => {
                let command_params: InjectTagFilterParams =
                    match serde_json::value::from_value(
                        params.arguments[0].clone(),
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let file = self.store.get_ast_file(
                    &command_params.text_document.uri,
                )?;
                let transformed = match transform::inject_tag_filter(
                    &file,
                    command_params.name,
                    command_params.bucket,
                ) {
                    Ok(value) => value,
                    Err(err) => {
                        return Err(LspError::InternalError(format!(
                            "{:?}",
                            err
                        ))
                        .into())
                    }
                };

                let new_text =
                    match flux::formatter::convert_to_string(
                        &transformed,
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let last_pos =
                    line_col::LineColLookup::new(&new_text)
                        .get(new_text.len());
                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: new_text.clone(),
                            range: lsp::Range {
                                start: lsp::Position::default(),
                                end: lsp::Position {
                                    line: last_pos.0 as u32,
                                    character: last_pos.1 as u32,
                                },
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };
                if let Some(client) = self.get_client() {
                    match client.apply_edit(edit, None).await {
                        Ok(response) => {
                            if response.applied {
                                self.store.put(
                                    &command_params.text_document.uri,
                                    &new_text,
                                );
                            }
                        }
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                }
                Ok(None)
            }
            Ok(LspServerCommand::InjectTagValueFilter) => {
                let command_params: InjectTagValueFilterParams =
                    match serde_json::value::from_value(
                        params.arguments[0].clone(),
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let file = self.store.get_ast_file(
                    &command_params.text_document.uri,
                )?;
                let transformed =
                    match transform::inject_tag_value_filter(
                        &file,
                        command_params.name,
                        command_params.value,
                        command_params.bucket,
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };

                let new_text =
                    match flux::formatter::convert_to_string(
                        &transformed,
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let last_pos =
                    line_col::LineColLookup::new(&new_text)
                        .get(new_text.len());
                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: new_text.clone(),
                            range: lsp::Range {
                                start: lsp::Position::default(),
                                end: lsp::Position {
                                    line: last_pos.0 as u32,
                                    character: last_pos.1 as u32,
                                },
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };
                if let Some(client) = self.get_client() {
                    match client.apply_edit(edit, None).await {
                        Ok(response) => {
                            if response.applied {
                                self.store.put(
                                    &command_params.text_document.uri,
                                    &new_text,
                                );
                            }
                        }
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                }
                Ok(None)
            }
            Ok(LspServerCommand::InjectFieldFilter) => {
                let command_params: InjectFieldFilterParams =
                    match serde_json::value::from_value(
                        params.arguments[0].clone(),
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let file = self.store.get_ast_file(
                    &command_params.text_document.uri,
                )?;
                let transformed = match transform::inject_field_filter(
                    &file,
                    command_params.name,
                    command_params.bucket,
                ) {
                    Ok(value) => value,
                    Err(err) => {
                        return Err(LspError::InternalError(format!(
                            "{:?}",
                            err
                        ))
                        .into())
                    }
                };

                let new_text =
                    match flux::formatter::convert_to_string(
                        &transformed,
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let last_pos =
                    line_col::LineColLookup::new(&new_text)
                        .get(new_text.len());
                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: new_text.clone(),
                            range: lsp::Range {
                                start: lsp::Position::default(),
                                end: lsp::Position {
                                    line: last_pos.0 as u32,
                                    character: last_pos.1 as u32,
                                },
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };
                if let Some(client) = self.get_client() {
                    match client.apply_edit(edit, None).await {
                        Ok(response) => {
                            if response.applied {
                                self.store.put(
                                    &command_params.text_document.uri,
                                    &new_text,
                                );
                            }
                        }
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                }
                Ok(None)
            }
            Ok(LspServerCommand::InjectMeasurementFilter) => {
                let command_params: InjectMeasurementFilterParams =
                    match serde_json::value::from_value(
                        params.arguments[0].clone(),
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let file = self.store.get_ast_file(
                    &command_params.text_document.uri,
                )?;
                let transformed =
                    match transform::inject_measurement_filter(
                        &file,
                        command_params.name,
                        command_params.bucket,
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };

                let new_text =
                    match flux::formatter::convert_to_string(
                        &transformed,
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                let last_pos =
                    line_col::LineColLookup::new(&new_text)
                        .get(new_text.len());
                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: new_text.clone(),
                            range: lsp::Range {
                                start: lsp::Position::default(),
                                end: lsp::Position {
                                    line: last_pos.0 as u32,
                                    character: last_pos.1 as u32,
                                },
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };
                if let Some(client) = self.get_client() {
                    match client.apply_edit(edit, None).await {
                        Ok(response) => {
                            if response.applied {
                                self.store.put(
                                    &command_params.text_document.uri,
                                    &new_text,
                                );
                            }
                        }
                        Err(err) => {
                            return Err(LspError::InternalError(
                                format!("{:?}", err),
                            )
                            .into())
                        }
                    };
                }
                Ok(None)
            }
            Ok(LspServerCommand::GetFunctionList) => {
                let functions: Vec<String> =
                    stdlib::get_builtin_functions()
                        .iter()
                        .filter(|function| {
                            !function.name.starts_with('_')
                        })
                        .map(|function| function.name.clone())
                        .collect();
                Ok(Some(functions.into()))
            }
            Err(_err) => {
                return Err(
                    LspError::InvalidCommand(params.command).into()
                )
            }
        }
    }
}

// Url::to_file_path doesn't exist in wasm-unknown-unknown, for kinda
// obvious reasons. Ignore these tests when executing against that target.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
