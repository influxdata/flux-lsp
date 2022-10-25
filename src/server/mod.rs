mod commands;
mod store;
mod types;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use flux::ast::walk::Node as AstNode;
use flux::ast::{self, Expression as AstExpression};
use flux::semantic::nodes::{
    ErrorKind as SemanticNodeErrorKind, Package as SemanticPackage,
};
use flux::semantic::sub::{Substitutable, Substituter};
use flux::semantic::types::{
    BoundTvar, BoundTvarKinds, BuiltinType, CollectionType, MonoType,
    PolyType, Tvar,
};
use flux::semantic::{walk, ErrorKind};
use lspower::{
    jsonrpc::Result as RpcResult, lsp, Client, LanguageServer,
};
use strum::IntoEnumIterator;

use crate::{completion, composition, lang, visitors::semantic};

use self::commands::{
    CompositionInitializeParams, LspServerCommand,
    TagValueFilterParams, ValueFilterParams,
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

        let scope: walk::Node = match path
            .iter()
            .map(|n| match n {
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
            })
            .next()
        {
            Some(Some(n)) => n.to_owned(),
            _ => return Vec::new(),
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

#[derive(Default)]
struct LspServerState {
    buckets: Vec<String>,
    compositions: HashMap<lsp::Url, composition::Composition>,
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

    /// Get a composition from the state
    ///
    /// We return a copy here, as the pointer across threads isn't supported.
    pub fn get_composition(
        &self,
        uri: &lsp::Url,
    ) -> Option<composition::Composition> {
        self.compositions.get(uri).cloned()
    }

    pub fn set_composition(
        &mut self,
        uri: lsp::Url,
        composition: composition::Composition,
    ) {
        self.compositions.insert(uri, composition);
    }

    pub fn drop_composition(&mut self, uri: &lsp::Url) {
        self.compositions.remove(uri);
    }
}

pub struct LspServer {
    client: Arc<Mutex<Option<Client>>>,
    diagnostics: Vec<Diagnostic>,
    store: store::Store,
    state: Mutex<LspServerState>,
    client_capabilities: RwLock<lsp::ClientCapabilities>,
}

impl LspServer {
    pub fn new(client: Option<Client>) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            diagnostics: vec![
                super::diagnostics::contrib_lint,
                super::diagnostics::experimental_lint,
                super::diagnostics::no_influxdb_identifiers,
                super::diagnostics::prefer_camel_case,
            ],
            store: store::Store::default(),
            state: Mutex::new(LspServerState::default()),
            client_capabilities: RwLock::new(
                lsp::ClientCapabilities::default(),
            ),
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

    fn complete_member_expression(
        &self,
        sem_pkg: &SemanticPackage,
        member: &ast::MemberExpr,
    ) -> Option<Vec<lsp::CompletionItem>> {
        match &member.object {
            AstExpression::Identifier(identifier) => {
                // XXX: rockstar (6 Jul 2022) - This is the last holdout from the previous
                // completion code. There is a bit of indirection/cruft here that can be cleaned
                // up when recursive support for member expressions is implemented.
                let mut list: Vec<Box<dyn completion::Completable>> =
                    vec![];
                if let Some(import) = completion::get_imports(sem_pkg)
                    .iter()
                    .find(|x| x.name == identifier.name)
                {
                    for package in lang::STDLIB.packages() {
                        if package.path == import.path {
                            completion::walk_package(
                                &package.path,
                                &mut list,
                                &package.exports.typ().expr,
                            );
                        }
                    }
                } else {
                    for package in lang::STDLIB.packages() {
                        if package.name == identifier.name {
                            completion::walk_package(
                                &package.path,
                                &mut list,
                                &package.exports.typ().expr,
                            );
                        }
                    }
                }

                let visitor = crate::walk_semantic_package!(
                    completion::CompletableObjectFinderVisitor::new(
                        &identifier.name
                    ),
                    sem_pkg
                );
                let imports = completion::get_imports(sem_pkg);
                Some(
                    vec![
                        visitor
                            .completables
                            .iter()
                            .map(|completable| {
                                completable.completion_item(&imports)
                            })
                            .collect::<Vec<lsp::CompletionItem>>(),
                        list.iter()
                            .map(|completable| {
                                completable.completion_item(&imports)
                            })
                            .collect(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                )
            }
            _ => None,
        }
    }
}

#[lspower::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(
        &self,
        params: lsp::InitializeParams,
    ) -> RpcResult<lsp::InitializeResult> {
        match self.client_capabilities.write() {
            Ok(mut client_capabilities) => {
                *client_capabilities = params.capabilities;
            }
            Err(err) => log::error!("{}", err),
        }

        Ok(lsp::InitializeResult {
            capabilities: lsp::ServerCapabilities {
                code_action_provider: Some(lsp::CodeActionProviderCapability::Simple(true)),
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
                definition_provider: Some(lsp::OneOf::Left(true)),
                document_formatting_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                document_highlight_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                document_symbol_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                execute_command_provider: Some(lsp::ExecuteCommandOptions {
                    commands: commands::LspServerCommand::iter().map(|command| command.into()).collect::<Vec<String>>(),
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: None,
                    }
                }),
                folding_range_provider: Some(
                    lsp::FoldingRangeProviderCapability::Simple(true),
                ),
                hover_provider: Some(
                    lsp::HoverProviderCapability::Simple(true),
                ),
                references_provider: Some(lsp::OneOf::Left(true)),
                rename_provider: Some(lsp::OneOf::Left(true)),
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
                    lsp::TextDocumentSyncCapability::Options(
                        lsp::TextDocumentSyncOptions {
                            open_close: Some(true),
                            change: Some(lsp::TextDocumentSyncKind::FULL),
                            ..Default::default()
                        }
                    ),
                ),
                ..Default::default()
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
                // The way the spec reads, if given a list of changes to make, these changes
                // are made in the order that they are provided, e.g. an straight iteration,
                // applying each one as given, is the correct process. That means a change later
                // in the list could overwrite a change made earlier in the list.
                let new_contents = params
                    .content_changes
                    .iter()
                    .fold(value, |_acc, change| change.text.clone());
                self.store.put(&key, &new_contents.clone());
                self.publish_diagnostics(&key).await;

                match self.state.lock() {
                    Ok(mut state) => {
                        if let Some(mut composition) =
                            state.get_composition(&key)
                        {
                            match self.store.get_ast_file(&key) {
                            Ok(file) => {
                                let result = composition.resolve_with_ast(file);
                                if result.is_err() {
                                    state.drop_composition(&key);
                                    if let Some(client) = &self.get_client() {
                                        let _ = client.show_message(lsp::MessageType::ERROR, "A conflict has occured in the query composition. The composition has been aborted.");
                                    }
                                }
                            }
                            Err(_) => log::error!("Found composition but did not find ast for key: {}", key),
                        }
                        }
                    }
                    Err(err) => panic!("{}", err),
                }
            }
            Err(err) => log::error!(
                "Could not update key: {}\n{:?}",
                key,
                err
            ),
        }
    }

    async fn did_close(
        &self,
        params: lsp::DidCloseTextDocumentParams,
    ) -> () {
        self.store.remove(&params.text_document.uri);
        match self.state.lock() {
            Ok(mut state) => {
                state.drop_composition(&params.text_document.uri)
            }
            Err(err) => panic!("{}", err),
        }
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

        let visitor = crate::walk_semantic_package!(
            semantic::NodeFinderVisitor::new(
                params.text_document_position_params.position
            ),
            pkg
        );
        let signatures: Vec<lsp::SignatureInformation> = if let Some(
            node,
        ) =
            visitor.node
        {
            if let walk::Node::CallExpr(call) = node {
                let callee = call.callee.clone();

                if let flux::semantic::nodes::Expression::Member(member) = callee.clone() {
                    let name = member.property.clone();
                    if let flux::semantic::nodes::Expression::Identifier(ident) = member.object.clone() {
                        match lang::STDLIB.package(&ident.name) {
                            None => return Ok(None),
                            Some(package) => match package.function(&name) {
                                None => return Ok(None),
                                Some(function) => function.signature_information(),
                            }
                        }
                    } else {
                        return Ok(None);
                    }
                } else if let flux::semantic::nodes::Expression::Identifier(ident) = callee {
                    match lang::UNIVERSE.function(&ident.name) {
                        Some(function) => {
                            function.signature_information()
                        }
                        None => return Ok(None),
                    }
                } else {
                    log::debug!("signature_help on non-member and non-identifier");
                    return Ok(None);
                }
            } else {
                log::debug!("signature_help on non-call expression");
                return Ok(None);
            }
        } else {
            return Ok(None);
        };

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

        let visitor = crate::walk_semantic_package!(
            semantic::FoldFinderVisitor::default(),
            pkg
        );
        let results: Vec<lsp::FoldingRange> = visitor
            .nodes
            .into_iter()
            .map(|node| lsp::FoldingRange {
                start_line: node.loc().start.line,
                start_character: Some(node.loc().start.column),
                end_line: node.loc().end.line,
                end_character: Some(node.loc().end.column),
                kind: Some(lsp::FoldingRangeKind::Region),
            })
            .collect();

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

        let visitor = crate::walk_semantic_package!(
            semantic::SymbolsVisitor::new(key),
            pkg
        );
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

        let visitor = crate::walk_semantic_package!(
            semantic::NodeFinderVisitor::new(
                params.text_document_position_params.position
            ),
            pkg
        );
        if let Some(node) = visitor.node {
            let node_name = match node {
                walk::Node::Identifier(ident) => &ident.name,
                walk::Node::IdentifierExpr(ident) => &ident.name,
                _ => return Ok(None),
            };

            let definition_visitor = crate::walk_semantic_package!(
                semantic::DefinitionFinderVisitor::new(
                    node_name.clone()
                ),
                pkg
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

        let visitor = crate::walk_semantic_package!(
            semantic::NodeFinderVisitor::new(
                params.text_document_position.position
            ),
            pkg
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

        Ok(Some(lsp::WorkspaceEdit {
            changes: Some(HashMap::from([(key, edits)])),
            document_changes: None,
            change_annotations: None,
        }))
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

        let visitor = crate::walk_semantic_package!(
            semantic::NodeFinderVisitor::new(
                params.text_document_position_params.position
            ),
            pkg
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

        let visitor = crate::walk_semantic_package!(
            semantic::NodeFinderVisitor::new(
                params.text_document_position.position
            ),
            pkg
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

        let visitor = crate::walk_semantic_package!(
            semantic::NodeFinderVisitor::new(
                params.text_document_position_params.position
            ),
            pkg
        );
        if let Some(node) = visitor.node {
            let path = &visitor.path;
            let hover_type = node
                .type_of()
                .map(|t| include_constraints(path, t).to_string())
                .or_else(|| match node {
                    walk::Node::Identifier(ident) => {
                        // We hovered over an identifier without an attached type, try to figure
                        // it out from its context
                        let parent = path.get(path.len() - 2)?;
                        match parent {
                            // The type of assigned variables is the type of the right hand side
                            walk::Node::VariableAssgn(var) => {
                                Some(var.init.type_of().to_string())
                            }
                            walk::Node::MemberAssgn(var) => {
                                Some(var.init.type_of().to_string())
                            }
                            walk::Node::BuiltinStmt(builtin) => {
                                Some(builtin.typ_expr.to_string())
                            }

                            // The type of an property identifier is the type of the value
                            walk::Node::Property(property) => Some(
                                property.value.type_of().to_string(),
                            ),

                            // The type Function parameters can be derived from the function type
                            // stored in the function expression
                            walk::Node::FunctionParameter(_) => {
                                let func =
                                    path.get(path.len() - 3)?;
                                match func {
                                    walk::Node::FunctionExpr(
                                        func,
                                    ) => func
                                        .typ
                                        .parameter(
                                            ident.name.as_str(),
                                        )
                                        .map(|t| t.to_string()),
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                });
            if let Some(typ) = hover_type {
                let supports_markdown = match self
                    .client_capabilities
                    .read()
                {
                    Ok(client_capabilities) => {
                        if let Some(text_document) =
                            (*client_capabilities)
                                .text_document
                                .as_ref()
                        {
                            if let Some(hover) =
                                text_document.hover.as_ref()
                            {
                                hover.content_format.as_ref().map_or(false, |formats| formats.contains(&lsp::MarkupKind::Markdown))
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    Err(err) => {
                        log::error!("{}", err);
                        false
                    }
                };
                let hover_contents: lsp::HoverContents =
                    match supports_markdown {
                        true => lsp::HoverContents::Markup(
                            lsp::MarkupContent {
                                kind: lsp::MarkupKind::Markdown,
                                value: format!(
                                    "```flux\n{}\n```",
                                    typ
                                ),
                            },
                        ),
                        false => lsp::HoverContents::Scalar(
                            lsp::MarkedString::String(typ),
                        ),
                    };

                return Ok(Some(lsp::Hover {
                    contents: hover_contents,
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
        // This is the rules for matching whether a string should be part of
        // the completion matching.
        let fuzzy_match = |haystack: &str, needle: &str| -> bool {
            return haystack
                .to_lowercase()
                .contains(needle.to_lowercase().as_str());
        };

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

        let visitor = crate::walk_ast_package!(
            crate::visitors::ast::NodeFinderVisitor::new(
                params.text_document_position.position
            ),
            ast_pkg
        );
        let items = match visitor.node {
            Some(walk_node) => match walk_node.node {
                AstNode::CallExpr(call) => {
                    completion::complete_call_expr(
                        &params, &sem_pkg, call,
                    )
                }
                AstNode::Identifier(identifier) => {
                    match walk_node
                        .parent
                        .as_ref()
                        .map(|node| &node.node)
                    {
                        // The identifier is a member property so do member completion
                        Some(AstNode::MemberExpr(member))
                            if member
                                .property
                                .base()
                                .location
                                .start
                                == identifier.base.location.start =>
                        {
                            match self.complete_member_expression(
                                &sem_pkg, member,
                            ) {
                                Some(items) => items,
                                None => return Ok(None),
                            }
                        }
                        _ => {
                            // XXX: rockstar (6 Jul 2022) - This is helping to complete packages that
                            // have never been imported. That's probably not a great pattern.
                            let stdlib_completions: Vec<
                                lsp::CompletionItem,
                            > = lang::STDLIB
                                .fuzzy_matches(&identifier.name)
                                .map(|package| {
                                    lsp::CompletionItem {
                                label: package.path.clone(),
                                detail: Some("Package".into()),
                                documentation: Some(
                                    lsp::Documentation::String(
                                        package.path.clone(),
                                    ),
                                ),
                                filter_text: Some(
                                    package.name.clone(),
                                ),
                                insert_text: Some(
                                    package.path.clone(),
                                ),
                                insert_text_format: Some(
                                    lsp::InsertTextFormat::PLAIN_TEXT,
                                ),
                                kind: Some(
                                    lsp::CompletionItemKind::MODULE,
                                ),
                                sort_text: Some(package.path),
                                ..lsp::CompletionItem::default()
                            }
                                })
                                .collect();

                            let builtin_completions: Vec<
                        lsp::CompletionItem,
                    > = lang::UNIVERSE.exports.iter().filter(|(key, val)| {
                            // Don't allow users to "discover" private-ish functionality.
                            // Filter out irrelevent items that won't match.
                            // Only pass expressions that have completion support.
                            !key.starts_with('_') && fuzzy_match(key, &identifier.name) &&
                            match &val.expr {
                                MonoType::Fun(_) | MonoType::Builtin(_) => true,
                                MonoType::Collection(collection) => collection.collection == CollectionType::Array,
                                _ => false
                            }
                        }).map(|(key, val)| {
                            match &val.expr {
                                MonoType::Fun(function) => {
                                    lsp::CompletionItem {
                                        label: key.to_string(),
                                        detail: Some(completion::create_function_signature(function)),
                                        filter_text: Some(key.to_string()),
                                        insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
                                        kind: Some(lsp::CompletionItemKind::FUNCTION),
                                        sort_text: Some(key.to_string()),
                                        ..lsp::CompletionItem::default()
                                    }
                                }
                                MonoType::Builtin(builtin) => {
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
                                        documentation: Some(lsp::Documentation::String("from prelude".into())),
                                        filter_text: Some(key.to_string()),
                                        insert_text: Some(key.to_string()),
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
                        }).collect();

                            vec![
                                stdlib_completions,
                                builtin_completions,
                            ]
                            .into_iter()
                            .flatten()
                            .collect()
                        }
                    }
                }
                AstNode::MemberExpr(member) => {
                    match self
                        .complete_member_expression(&sem_pkg, member)
                    {
                        Some(items) => items,
                        None => return Ok(None),
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
                        Some(_) | None => return Ok(None),
                    }
                }
                AstNode::StringLit(_) => {
                    let parent = walk_node
                        .parent
                        .as_ref()
                        .map(|parent| &parent.node);
                    match parent {
                        Some(AstNode::ImportDeclaration(_)) => {
                            let imports =
                                completion::get_imports(&sem_pkg);

                            lang::STDLIB.packages().filter(|package| {
                                !&imports.iter().any(|x| x.path == package.path)
                            }).map(|package| {
                                let trigger = if let Some(context) = & params.context {
                                    context.trigger_character.as_deref()
                                } else {
                                    None
                                };
                                let insert_text = if trigger == Some("\"") {
                                    package.path.as_str().to_string()
                                } else {
                                    format!(r#""{}""#, package.path.as_str())
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
                        Some(_) | None => return Ok(None),
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

        let visitor = crate::walk_ast_package!(
            crate::visitors::ast::SemanticTokenVisitor::default(),
            pkg
        );
        Ok(Some(lsp::SemanticTokensResult::Tokens(
            lsp::SemanticTokens {
                result_id: None,
                data: visitor.tokens,
            },
        )))
    }

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

        let visitor = crate::walk_semantic_package!(
            semantic::PackageNodeFinderVisitor::default(),
            pkg
        );
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
                        // When encountering undefined identifiers, check to see if they match any corresponding
                        // packages available for import.
                        let potential_imports: Vec<lang::Package> = lang::STDLIB.fuzzy_matches(identifier).collect();
                        if potential_imports.is_empty() {
                            return None;
                        }

                        let inner_actions: Vec<lsp::CodeActionOrCommand> = potential_imports.iter().map(|package| {
                            lsp::CodeAction {
                                title: format!("Import `{}`", package.path),
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
                                                new_text: format!("import \"{}\"\n", package.path),
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
            Ok(LspServerCommand::CompositionInitialize) => {
                let command_params: CompositionInitializeParams =
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
                let composition = composition::Composition::new(
                    file,
                    command_params.bucket,
                    command_params.measurement,
                    command_params.fields.unwrap_or_default(),
                    command_params.tag_values.unwrap_or_default(),
                );

                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: composition.to_string(),
                            range: {
                                let file = self.store.get_ast_file(
                                    &command_params.text_document.uri,
                                )?;
                                file.base.location.into()
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };

                match self.state.lock() {
                    Ok(mut state) => state.set_composition(
                        command_params.text_document.uri,
                        composition,
                    ),
                    Err(err) => panic!("{}", err),
                }
                if let Some(client) = self.get_client() {
                    let _ = client.apply_edit(edit, None).await;
                };
                Ok(None)
            }
            Ok(LspServerCommand::AddMeasurementFilter) => {
                let command_params: ValueFilterParams =
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

                let mut composition = match self.state.lock() {
                    Ok(state) => match state.get_composition(
                        &command_params.text_document.uri,
                    ) {
                        Some(composition) => composition,
                        None => {
                            return Err(
                                LspError::CompositionNotFound(
                                    command_params.text_document.uri,
                                )
                                .into(),
                            )
                        }
                    },
                    Err(err) => panic!("{}", err),
                };

                if let Err(_) =
                    composition.add_measurement(command_params.value)
                {
                    return Err(LspError::InternalError(
                        "Failed to add measurement to composition."
                            .to_string(),
                    )
                    .into());
                }

                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: composition.to_string(),
                            range: {
                                let file = self.store.get_ast_file(
                                    &command_params.text_document.uri,
                                )?;
                                file.base.location.into()
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };

                match self.state.lock() {
                    Ok(mut state) => state.set_composition(
                        command_params.text_document.uri,
                        composition,
                    ),
                    Err(err) => panic!("{}", err),
                }
                if let Some(client) = self.get_client() {
                    let _ = client.apply_edit(edit, None).await;
                };
                Ok(None)
            }
            Ok(LspServerCommand::AddFieldFilter) => {
                let command_params: ValueFilterParams =
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

                let mut composition = match self.state.lock() {
                    Ok(state) => match state.get_composition(
                        &command_params.text_document.uri,
                    ) {
                        Some(composition) => composition,
                        None => {
                            return Err(
                                LspError::CompositionNotFound(
                                    command_params.text_document.uri,
                                )
                                .into(),
                            )
                        }
                    },
                    Err(err) => panic!("{}", err),
                };

                if let Err(_) =
                    composition.add_field(command_params.value)
                {
                    return Err(LspError::InternalError(
                        "Failed to add field to composition."
                            .to_string(),
                    )
                    .into());
                }

                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: composition.to_string(),
                            range: {
                                let file = self.store.get_ast_file(
                                    &command_params.text_document.uri,
                                )?;
                                file.base.location.into()
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };

                match self.state.lock() {
                    Ok(mut state) => state.set_composition(
                        command_params.text_document.uri,
                        composition,
                    ),
                    Err(err) => panic!("{}", err),
                }
                if let Some(client) = self.get_client() {
                    let _ = client.apply_edit(edit, None).await;
                };
                Ok(None)
            }
            Ok(LspServerCommand::RemoveFieldFilter) => {
                let command_params: ValueFilterParams =
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

                let mut composition = match self.state.lock() {
                    Ok(state) => match state.get_composition(
                        &command_params.text_document.uri,
                    ) {
                        Some(composition) => composition,
                        None => {
                            return Err(
                                LspError::CompositionNotFound(
                                    command_params.text_document.uri,
                                )
                                .into(),
                            )
                        }
                    },
                    Err(err) => panic!("{}", err),
                };

                if let Err(_) =
                    composition.remove_field(command_params.value)
                {
                    return Err(LspError::InternalError(
                        "Failed to remove field from composition."
                            .to_string(),
                    )
                    .into());
                }

                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: composition.to_string(),
                            range: {
                                let file = self.store.get_ast_file(
                                    &command_params.text_document.uri,
                                )?;
                                file.base.location.into()
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };

                match self.state.lock() {
                    Ok(mut state) => state.set_composition(
                        command_params.text_document.uri,
                        composition,
                    ),
                    Err(err) => panic!("{}", err),
                }
                if let Some(client) = self.get_client() {
                    let _ = client.apply_edit(edit, None).await;
                };
                Ok(None)
            }
            Ok(LspServerCommand::AddTagValueFilter) => {
                let command_params: TagValueFilterParams =
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

                let mut composition = match self.state.lock() {
                    Ok(state) => match state.get_composition(
                        &command_params.text_document.uri,
                    ) {
                        Some(composition) => composition,
                        None => {
                            return Err(
                                LspError::CompositionNotFound(
                                    command_params.text_document.uri,
                                )
                                .into(),
                            )
                        }
                    },
                    Err(err) => panic!("{}", err),
                };

                if let Err(_) = composition.add_tag_value(
                    command_params.tag,
                    command_params.value,
                ) {
                    return Err(LspError::InternalError(
                        "Failed to add tagValue to composition."
                            .to_string(),
                    )
                    .into());
                }

                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: composition.to_string(),
                            range: {
                                let file = self.store.get_ast_file(
                                    &command_params.text_document.uri,
                                )?;
                                file.base.location.into()
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };

                match self.state.lock() {
                    Ok(mut state) => state.set_composition(
                        command_params.text_document.uri,
                        composition,
                    ),
                    Err(err) => panic!("{}", err),
                }
                if let Some(client) = self.get_client() {
                    let _ = client.apply_edit(edit, None).await;
                };
                Ok(None)
            }
            Ok(LspServerCommand::RemoveTagValueFilter) => {
                let command_params: TagValueFilterParams =
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

                let mut composition = match self.state.lock() {
                    Ok(state) => match state.get_composition(
                        &command_params.text_document.uri,
                    ) {
                        Some(composition) => composition,
                        None => {
                            return Err(
                                LspError::CompositionNotFound(
                                    command_params.text_document.uri,
                                )
                                .into(),
                            )
                        }
                    },
                    Err(err) => panic!("{}", err),
                };

                if let Err(_) = composition.remove_tag_value(
                    command_params.tag,
                    command_params.value,
                ) {
                    return Err(LspError::InternalError(
                        "Failed to remove tagValue from composition."
                            .to_string(),
                    )
                    .into());
                }

                let edit = lsp::WorkspaceEdit {
                    changes: Some(HashMap::from([(
                        command_params.text_document.uri.clone(),
                        vec![lsp::TextEdit {
                            new_text: composition.to_string(),
                            range: {
                                let file = self.store.get_ast_file(
                                    &command_params.text_document.uri,
                                )?;
                                file.base.location.into()
                            },
                        }],
                    )])),
                    document_changes: None,
                    change_annotations: None,
                };

                match self.state.lock() {
                    Ok(mut state) => state.set_composition(
                        command_params.text_document.uri,
                        composition,
                    ),
                    Err(err) => panic!("{}", err),
                }
                if let Some(client) = self.get_client() {
                    let _ = client.apply_edit(edit, None).await;
                };
                Ok(None)
            }
            Ok(LspServerCommand::GetFunctionList) => Ok(Some(
                lang::UNIVERSE
                    .functions()
                    .iter()
                    .map(|function| function.name.clone())
                    .collect(),
            )),
            Err(_err) => {
                return Err(
                    LspError::InvalidCommand(params.command).into()
                )
            }
        }
    }
}

// `MonoType`'s extracted from a `Node` in a semantic graph do not contain the constraints directly
// on them however we can locate the parent variable assignment to the type (`t`) and figure out
// which constraints apply.
fn include_constraints(
    path: &[walk::Node<'_>],
    t: MonoType,
) -> PolyType {
    // Get all constraints that may apply to `t`
    let all_constraints =
        path.iter().rev().find_map(|parent| match parent {
            walk::Node::VariableAssgn(assgn) => {
                Some(assgn.poly_type_of().cons)
            }
            _ => None,
        });

    let mut constraints = BoundTvarKinds::default();
    if let Some(all_constraints) = all_constraints {
        // Pick out the constraints that apply to `t`
        t.visit(&mut VisitBoundVars(|var| {
            if let Some(c) = all_constraints.get(&var) {
                constraints.entry(var).or_insert_with(|| c.clone());
            }
        }));
    }
    PolyType {
        vars: Vec::new(),
        cons: constraints,
        expr: t,
    }
}

struct VisitBoundVars<F>(F);
impl<F> Substituter for VisitBoundVars<F>
where
    F: FnMut(BoundTvar),
{
    fn try_apply(&mut self, _var: Tvar) -> Option<MonoType> {
        None
    }

    fn try_apply_bound(
        &mut self,
        var: BoundTvar,
    ) -> Option<MonoType> {
        (self.0)(var);
        None
    }
}

// Url::to_file_path doesn't exist in wasm-unknown-unknown, for kinda
// obvious reasons. Ignore these tests when executing against that target.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
