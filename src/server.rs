use std::borrow::Cow;
use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use flux::analyze;
use flux::ast::walk::walk_rc;
use flux::ast::walk::Node as AstNode;
use flux::ast::{Expression, Package, PropertyKey, SourceLocation};
use flux::parser::parse_string;
use flux::semantic::nodes::Expression as SemanticExpression;
use flux::semantic::nodes::{CallExpr, FunctionParameter};
use flux::semantic::types::{MonoType, Record};
use flux::semantic::walk;
use flux::semantic::walk::Node as SemanticNode;
use flux::semantic::walk::Visitor as SemanticVisitor;
use flux::{imports, prelude};
use log::{debug, error, info, warn};
use lspower::jsonrpc::Result as RpcResult;
use lspower::lsp;
use lspower::LanguageServer;

use crate::convert;
use crate::shared::Function;
use crate::shared::{get_argument_names, FunctionSignature};
use crate::shared::{get_package_name, is_in_node};
use crate::stdlib::{
    create_function_signature, get_builtin_functions,
    get_package_functions, get_package_infos, get_stdlib_functions,
};
use crate::visitors::ast::{
    CallFinderVisitor, NodeFinderVisitor, PackageFinderVisitor,
    PackageInfo,
};
use crate::visitors::semantic::NodeFinderVisitor as SemanticNodeFinderVisitor;
use crate::visitors::semantic::{
    DefinitionFinderVisitor, FoldFinderVisitor,
    FunctionFinderVisitor, IdentFinderVisitor, Import,
    ImportFinderVisitor, ObjectFunctionFinderVisitor, SymbolsVisitor,
};

const PRELUDE_PACKAGE: &str = "prelude";

// The spec talks specifically about setting versions for files, but isn't
// clear on how those versions are surfaced to the client, if ever. This
// type could be extended to keep track of versions of files, but simplicity
// is preferred at this point.
type FileStore = Arc<Mutex<HashMap<lsp::Url, String>>>;

fn parse_and_analyze(
    code: &str,
) -> Result<flux::semantic::nodes::Package> {
    let mut analyzer = flux::new_semantic_analyzer(
        flux::semantic::AnalyzerConfig {
            // Explicitly disable the AST and Semantic checks.
            // We do not care if the code is syntactically or semantically correct as this may be
            // partially written code.
            skip_checks: true,
        },
    )?;
    let (_, sem_pkg) = analyzer.analyze_source(
        "".to_string(),
        "main.flux".to_string(),
        code,
    )?;
    Ok(sem_pkg)
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
        error!("range end not found after range start");
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

fn is_scope(name: &str, n: SemanticNode<'_>) -> bool {
    let mut dvisitor = DefinitionFinderVisitor::new(name.to_string());
    walk::walk(&mut dvisitor, n.clone());
    let state = dvisitor.state.borrow();

    state.node.is_some()
}

fn find_references(
    uri: lsp::Url,
    result: NodeFinderResult,
) -> Vec<lsp::Location> {
    if let Some(node) = result.node {
        let name = match node {
            SemanticNode::Identifier(ident) => ident.name.as_str(),
            SemanticNode::IdentifierExpr(ident) => {
                ident.name.as_str()
            }
            _ => return Vec::new(),
        };

        let mut path_iter = result.path.iter().rev();
        let scope: SemanticNode =
            match path_iter.find_map(|n| match n {
                SemanticNode::FunctionExpr(f)
                    if function_defines(name, &f.params) =>
                {
                    Some(n)
                }
                SemanticNode::Package(_) | walk::Node::File(_)
                    if is_scope(name, n.clone()) =>
                {
                    Some(n)
                }
                _ => None,
            }) {
                Some(n) => n.to_owned(),
                None => return Vec::new(),
            };

        let mut visitor = IdentFinderVisitor::new(name.to_string());
        walk::walk(&mut visitor, scope);

        let state = visitor.state.borrow();

        let locations: Vec<lsp::Location> = (*state)
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
    name: String,
    package: String,
) -> Vec<lsp::SignatureInformation> {
    get_stdlib_functions()
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
                        lsp::TextDocumentSyncKind::Full,
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
        Ok(())
    }
    async fn did_open(
        &self,
        params: lsp::DidOpenTextDocumentParams,
    ) -> () {
        let key = params.text_document.uri;
        let value = params.text_document.text;
        let mut store = match self.store.lock() {
            Ok(value) => value,
            Err(err) => {
                warn!("Could not acquire store lock. Error: {}", err);
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
                warn!(
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
        let mut store = match self.store.lock() {
            Ok(value) => value,
            Err(err) => {
                warn!("Could not acquire store lock. Error: {}", err);
                return;
            }
        };
        let mut contents = if let Some(contents) = store.get(&key) {
            Cow::Borrowed(contents)
        } else {
            error!(
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
        store.insert(key.clone(), new_contents);
    }
    async fn did_save(
        &self,
        params: lsp::DidSaveTextDocumentParams,
    ) -> () {
        if let Some(text) = params.text {
            let key = params.text_document.uri;
            let mut store = match self.store.lock() {
                Ok(value) => value,
                Err(err) => {
                    warn!(
                        "Could not acquire store lock. Error: {}",
                        err
                    );
                    return;
                }
            };
            if !store.contains_key(&key) {
                warn!(
                    "textDocument/didSave called on unknown file {}",
                    key
                );
                return;
            }
            store.insert(key, text);
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
                warn!("Could not acquire store lock. Error: {}", err);
                return;
            }
        };
        if store.remove(&key).is_none() {
            // The protocol spec is unclear on whether trying to close a file
            // that isn't open is allowed. To stop consistent with the
            // implementation of textDocument/didOpen, this error is logged and
            // allowed.
            warn!(
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
        let pkg = {
            let store = match self.store.lock() {
                Ok(value) => value,
                Err(err) => {
                    return Err(lspower::jsonrpc::Error {
                        code:
                            lspower::jsonrpc::ErrorCode::InternalError,
                        message: format!(
                            "Could not acquire store lock. Error: {}",
                            err
                        ),
                        data: None,
                    });
                }
            };
            let data = store.get(&key).ok_or_else(|| {
                // File isn't loaded into memory
                error!(
                    "signature help failed: file {} not open on server",
                    key
                );
                file_not_opened(&key)
            })?;

            match parse_and_analyze(data) {
                Ok(pkg) => pkg,
                Err(err) => {
                    debug!("{}", err);
                    return Ok(None);
                }
            }
        };

        let mut signatures = vec![];
        let node_finder_result = find_node(
            SemanticNode::Package(&pkg),
            params.text_document_position_params.position,
        );

        if let Some(node) = node_finder_result.node {
            if let SemanticNode::CallExpr(call) = node {
                let callee = call.callee.clone();

                if let flux::semantic::nodes::Expression::Member(member) = callee.clone() {
                    let name = member.property.clone();
                    if let flux::semantic::nodes::Expression::Identifier(ident) = member.object.clone() {
                        signatures.extend(find_stdlib_signatures(name, ident.name.to_string()));
                    }
                } else if let flux::semantic::nodes::Expression::Identifier(ident) = callee {
                    signatures.extend(find_stdlib_signatures(
                            ident.name.to_string(),
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
    ) -> RpcResult<Option<Vec<lsp::TextEdit>>> {
        let key = params.text_document.uri;

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
        let contents = store.get(&key).ok_or_else(|| {
            error!(
                "formatting failed: file {} not open on server",
                key
            );
            file_not_opened(&key)
        })?;
        let mut formatted = match flux::formatter::format(contents) {
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
                info!("textDocument/formatting requested trimming trailing whitespace, but the flux formatter will always trim trailing whitespace");
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
                info!("textDocument/formatting requested trimming final newlines, but the flux formatter will always trim trailing whitespace");
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
        let pkg = {
            let store = match self.store.lock() {
                Ok(value) => value,
                Err(err) => {
                    return Err(lspower::jsonrpc::Error {
                        code:
                            lspower::jsonrpc::ErrorCode::InternalError,
                        message: format!(
                            "Could not acquire store lock. Error: {}",
                            err
                        ),
                        data: None,
                    });
                }
            };
            let contents = store.get(&key).ok_or_else(|| {
                error!(
                    "formatting failed: file {} not open on server",
                    key
                );
                file_not_opened(&key)
            })?;
            match parse_and_analyze(contents.as_str()) {
                Ok(pkg) => pkg,
                Err(err) => {
                    debug!("{}", err);
                    return Ok(None);
                }
            }
        };
        let mut visitor = FoldFinderVisitor::default();
        let pkg_node = SemanticNode::Package(&pkg);

        walk::walk(&mut visitor, pkg_node);

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
    ) -> RpcResult<Option<lsp::DocumentSymbolResponse>> {
        let key = params.text_document.uri;
        let pkg = {
            let store = match self.store.lock() {
                Ok(value) => value,
                Err(err) => {
                    return Err(lspower::jsonrpc::Error {
                        code:
                            lspower::jsonrpc::ErrorCode::InternalError,
                        message: format!(
                            "Could not acquire store lock. Error: {}",
                            err
                        ),
                        data: None,
                    });
                }
            };
            let contents = store.get(&key).ok_or_else(|| {
                error!(
                    "documentSymbol request failed: file {} not open on server",
                    key,
                );
                file_not_opened(&key)
            })?;

            match parse_and_analyze(contents) {
                Ok(pkg) => pkg,
                Err(err) => {
                    debug!("{}", err);
                    return Ok(None);
                }
            }
        };
        let pkg_node = SemanticNode::Package(&pkg);
        let mut visitor = SymbolsVisitor::new(key);
        walk::walk(&mut visitor, pkg_node);

        let state = visitor.state.borrow();
        let mut symbols = (*state).symbols.clone();

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
        let contents = store.get(&key).ok_or_else(|| {
            error!(
                "formatting failed: file {} not open on server",
                key
            );
            file_not_opened(&key)
        })?;
        let pkg = match parse_and_analyze(contents) {
            Ok(pkg) => pkg,
            Err(err) => {
                debug!("{}", err);
                return Ok(None);
            }
        };
        let pkg_node = SemanticNode::Package(&pkg);
        let mut visitor = SemanticNodeFinderVisitor::new(
            params.text_document_position_params.position,
        );

        flux::semantic::walk::walk(&mut visitor, pkg_node);

        let state = visitor.state.borrow();
        let node = (*state).node.clone();
        let path = (*state).path.clone();

        if let Some(node) = node {
            let name = match node {
                SemanticNode::Identifier(ident) => {
                    Some(ident.name.clone())
                }
                SemanticNode::IdentifierExpr(ident) => {
                    Some(ident.name.clone())
                }
                _ => return Ok(None),
            };

            if let Some(node_name) = name {
                let path_iter = path.iter().rev();
                for n in path_iter {
                    match n {
                        SemanticNode::FunctionExpr(_)
                        | SemanticNode::Package(_)
                        | SemanticNode::File(_) => {
                            if let SemanticNode::FunctionExpr(f) = n {
                                for param in f.params.clone() {
                                    let name = param.key.name;
                                    if name != node_name {
                                        continue;
                                    }
                                    let location =
                                        convert::node_to_location(
                                            &node, key,
                                        );
                                    return Ok(Some(lsp::GotoDefinitionResponse::from(location)));
                                }
                            }

                            let mut definition_visitor: DefinitionFinderVisitor =
                                DefinitionFinderVisitor::new(node_name.to_string());

                            flux::semantic::walk::walk(
                                &mut definition_visitor,
                                n.clone(),
                            );

                            let state =
                                definition_visitor.state.borrow();
                            if let Some(node) = state.node.clone() {
                                let location =
                                    convert::node_to_location(
                                        &node, key,
                                    );
                                return Ok(Some(
                                    lsp::GotoDefinitionResponse::from(
                                        location,
                                    ),
                                ));
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
        Ok(None)
    }
    async fn rename(
        &self,
        params: lsp::RenameParams,
    ) -> RpcResult<Option<lsp::WorkspaceEdit>> {
        let key =
            params.text_document_position.text_document.uri.clone();
        let pkg = {
            let store = match self.store.lock() {
                Ok(value) => value,
                Err(err) => {
                    return Err(lspower::jsonrpc::Error {
                        code:
                            lspower::jsonrpc::ErrorCode::InternalError,
                        message: format!(
                            "Could not acquire store lock. Error: {}",
                            err
                        ),
                        data: None,
                    });
                }
            };
            let contents = store.get(&key).ok_or_else(|| {
                error!(
                    "textDocument/rename called on unknown file {}",
                    key
                );
                file_not_opened(&key)
            })?;
            match parse_and_analyze(contents) {
                Ok(pkg) => pkg,
                Err(err) => {
                    debug!("{}", err);
                    return Ok(None);
                }
            }
        };
        let node = find_node(
            SemanticNode::Package(&pkg),
            params.text_document_position.position,
        );

        let locations = find_references(key.clone(), node);
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
    async fn references(
        &self,
        params: lsp::ReferenceParams,
    ) -> RpcResult<Option<Vec<lsp::Location>>> {
        let key =
            params.text_document_position.text_document.uri.clone();
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
        let contents = store.get(&key).ok_or_else(|| {
            error!(
                "textDocument/references called on unknown file {}",
                key
            );
            file_not_opened(&key)
        })?;
        let pkg = match parse_and_analyze(contents) {
            Ok(pkg) => pkg,
            Err(err) => {
                debug!("{}", err);
                return Ok(None);
            }
        };
        let node = find_node(
            SemanticNode::Package(&pkg),
            params.text_document_position.position,
        );

        Ok(Some(find_references(key, node)))
    }
    // XXX: rockstar (9 Aug 2021) - This implementation exists here *solely* for
    // compatibility with the previous server. This behavior is identical to it,
    // although very clearly kinda useless.
    async fn hover(
        &self,
        _params: lsp::HoverParams,
    ) -> RpcResult<Option<lsp::Hover>> {
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
        let key =
            params.text_document_position.text_document.uri.clone();

        let contents = {
            let store = match self.store.lock() {
                Ok(value) => value,
                Err(err) => {
                    return Err(lspower::jsonrpc::Error {
                        code:
                            lspower::jsonrpc::ErrorCode::InternalError,
                        message: format!(
                            "Could not acquire store lock. Error: {}",
                            err
                        ),
                        data: None,
                    });
                }
            };
            store
                .get(&key)
                .ok_or_else(|| {
                    error!(
                        "textDocument/completion called on unknown file {}",
                        key
                    );
                    file_not_opened(&key)
                })?
                .to_string()
        };

        let items = if let Some(ctx) = params.context.clone() {
            match (ctx.trigger_kind, ctx.trigger_character) {
                (
                    lsp::CompletionTriggerKind::TriggerCharacter,
                    Some(c),
                ) => match c.as_str() {
                    "." => find_dot_completions(params, contents),
                    // XXX: sean (10 Aug 2021) - All `find_arg_completions` does is
                    // look for bucket names if the parameter name is "bucket". Since
                    // we don't currently support bucket completions, this match arm
                    // is a no-op.
                    ":" => {
                        find_arg_completions(params, contents).await
                    }
                    "(" | "," => find_param_completions(
                        Some(c),
                        params,
                        contents,
                    ),
                    _ => find_completions(params, contents),
                },
                _ => find_completions(params, contents),
            }
        } else {
            find_completions(params, contents)
        };

        let items = match items {
            Ok(items) => items,
            Err(e) => {
                error!("error getting completion items: {}", e.msg);
                return Err(lspower::jsonrpc::Error::invalid_params(
                    format!(
                        "error getting completion items: {}",
                        e.msg
                    ),
                ));
            }
        };

        let response = lsp::CompletionResponse::List(items);
        Ok(Some(response))
    }
}

fn file_not_opened(key: &lsp::Url) -> lspower::jsonrpc::Error {
    lspower::jsonrpc::Error::invalid_params(format!(
        "file not opened: {}",
        key
    ))
}

// Url::to_file_path doesn't exist in wasm-unknown-unknown, for kinda
// obvious reasons. Ignore these tests when executing against that target.
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(deprecated)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use async_std::test;
    use lspower::lsp;
    use lspower::LanguageServer;

    use super::LspServer;

    fn create_server() -> LspServer {
        LspServer::default()
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
                assert_eq!(symbols.len(), 38)
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
                    lsp::Position::new(8, 35),
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
    async fn test_hover() {
        let fluxscript = r#"import "strings"#;
        let server = create_server();
        open_file(&server, fluxscript.to_string()).await;

        let params = lsp::HoverParams {
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

        let result = server.hover(params).await.unwrap();

        assert!(result.is_none());
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
                    lsp::CompletionTriggerKind::TriggerCharacter,
                trigger_character: Some(".".to_string()),
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let len = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items.len(),
            _ => unreachable!(),
        };

        assert_eq!(2, len);
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
                trigger_kind: lsp::CompletionTriggerKind::Invoked,
                trigger_character: None,
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let got: HashSet<&str> =
            items.iter().map(|i| i.label.as_str()).collect();

        let want: HashSet<&str> = vec![
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
                    lsp::CompletionTriggerKind::TriggerCharacter,
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
                trigger_kind: lsp::CompletionTriggerKind::Invoked,
                trigger_character: None,
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let got: HashSet<&str> =
            items.iter().map(|i| i.label.as_str()).collect();

        let want: HashSet<&str> = vec![
            "_window",
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
                    lsp::CompletionTriggerKind::TriggerCharacter,
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
                    lsp::CompletionTriggerKind::TriggerCharacter,
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
                trigger_kind: lsp::CompletionTriggerKind::Invoked,
                trigger_character: None,
            }),
        };

        let result =
            server.completion(params.clone()).await.unwrap().unwrap();

        let items = match result.clone() {
            lsp::CompletionResponse::List(l) => l.items,
            _ => unreachable!(),
        };

        let got: HashSet<&str> =
            items.iter().map(|i| i.label.as_str()).collect();

        let want: HashSet<&str> = vec![
            "_fillEmpty",
            "_highestOrLowest",
            "_sortLimit",
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

        let result = server.signature_help(params).await.unwrap();

        assert!(result.is_none())
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

        let result = server.folding_range(params).await.unwrap();

        assert!(result.is_none());
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
        let result = server.document_symbol(params).await.unwrap();

        assert!(result.is_none());
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

        let result = server.goto_definition(params).await.unwrap();

        assert!(result.is_none());
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

        let result = server.references(params.clone()).await.unwrap();

        assert!(result.is_none());
    }
}

#[derive(Debug)]
struct Error {
    msg: String,
}
impl From<String> for Error {
    fn from(s: String) -> Error {
        Error { msg: s }
    }
}

fn find_completions(
    params: lsp::CompletionParams,
    contents: String,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri.clone();
    let info =
        CompletionInfo::create(params.clone(), contents.clone())?;

    let mut items: Vec<lsp::CompletionItem> = vec![];

    if let Some(info) = info {
        match info.completion_type {
            CompletionType::Generic => {
                let mut stdlib_matches = get_stdlib_matches(
                    info.ident.clone(),
                    info.clone(),
                );
                items.append(&mut stdlib_matches);

                let mut user_matches =
                    get_user_matches(info, contents)?;

                items.append(&mut user_matches);
            }
            CompletionType::Bad => {}
            CompletionType::CallProperty(_func) => {
                return find_param_completions(None, params, contents)
            }
            CompletionType::Import => {
                let infos = get_package_infos();

                let imports = get_imports_removed(
                    uri,
                    info.position,
                    contents,
                )?;

                let mut items = vec![];
                for info in infos {
                    if !(&imports).iter().any(|x| x.path == info.name)
                    {
                        items.push(new_string_arg_completion(
                            info.path,
                            get_trigger(params.clone()),
                        ));
                    }
                }

                return Ok(lsp::CompletionList {
                    is_incomplete: false,
                    items,
                });
            }
            CompletionType::ObjectMember(_obj) => {
                return find_dot_completions(params, contents);
            }
            _ => {}
        }
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items,
    })
}

#[derive(Clone)]
enum CompletionType {
    Generic,
    Logical(flux::ast::Operator),
    CallProperty(String),
    ObjectMember(String),
    Import,
    Bad,
}

#[derive(Clone)]
struct CompletionInfo {
    completion_type: CompletionType,
    ident: String,
    position: lsp::Position,
    uri: lsp::Url,
    imports: Vec<Import>,
    package: Option<PackageInfo>,
}

impl CompletionInfo {
    fn create(
        params: lsp::CompletionParams,
        source: String,
    ) -> Result<Option<CompletionInfo>, String> {
        let uri =
            params.text_document_position.text_document.uri.clone();
        let position = params.text_document_position.position;

        let pkg: Package =
            parse_string(uri.to_string(), source.as_str()).into();
        let walker = Rc::new(AstNode::File(&pkg.files[0]));
        let visitor = NodeFinderVisitor::new(move_back(position, 1));

        walk_rc(&visitor, walker);

        let package = PackageFinderVisitor::find_package(
            uri.clone(),
            source.clone(),
        )?;

        let state = visitor.state.borrow();
        let finder_node = (*state).node.clone();

        if let Some(finder_node) = finder_node {
            if let Some(parent) = finder_node.parent {
                match parent.node.as_ref() {
                    AstNode::MemberExpr(me) => {
                        if let Expression::Identifier(obj) =
                            me.object.clone()
                        {
                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::ObjectMember(
                                        obj.name.clone(),
                                    ),
                                ident: obj.name,
                                position,
                                uri: uri.clone(),
                                imports: get_imports_removed(
                                    uri, position, source,
                                )?,
                                package,
                            }));
                        }
                    }
                    AstNode::ImportDeclaration(_id) => {
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Import,
                            ident: "".to_string(),
                            position,
                            uri: uri.clone(),
                            imports: get_imports_removed(
                                uri, position, source,
                            )?,
                            package,
                        }));
                    }
                    AstNode::BinaryExpr(be) => {
                        match be.left.clone() {
                            Expression::Identifier(left) => {
                                let name = left.name;

                                return Ok(Some(CompletionInfo {
                                    completion_type:
                                        CompletionType::Logical(
                                            be.operator.clone(),
                                        ),
                                    ident: name,
                                    position,
                                    uri: uri.clone(),
                                    imports: get_imports(
                                        uri, position, source,
                                    )?,
                                    package,
                                }));
                            }
                            Expression::Member(left) => {
                                if let Expression::Identifier(ident) =
                                    left.object
                                {
                                    let key = match left.property {
                                        PropertyKey::Identifier(
                                            ident,
                                        ) => ident.name,
                                        PropertyKey::StringLit(
                                            lit,
                                        ) => lit.value,
                                    };

                                    let name = format!(
                                        "{}.{}",
                                        ident.name, key
                                    );

                                    return Ok(Some(CompletionInfo {
                                                            completion_type:
                                                                CompletionType::Logical(
                                                                    be.operator.clone(),
                                                                ),
                                                                ident: name,
                                                                position,
                                                                uri: uri.clone(),
                                                                imports: get_imports(
                                                                    uri, position, source,
                                                                )?,
                                                                package,
                                                        }));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }

                if let Some(grandparent) = parent.parent {
                    if let Some(greatgrandparent) = grandparent.parent
                    {
                        if let (
                            AstNode::Property(prop),
                            AstNode::ObjectExpr(_),
                            AstNode::CallExpr(call),
                        ) = (
                            parent.node.as_ref(),
                            grandparent.node.as_ref(),
                            greatgrandparent.node.as_ref(),
                        ) {
                            let name = match prop.key.clone() {
                                PropertyKey::Identifier(ident) => {
                                    ident.name
                                }
                                PropertyKey::StringLit(lit) => {
                                    lit.value
                                }
                            };

                            if let Expression::Identifier(func) =
                                call.callee.clone()
                            {
                                return Ok(Some(CompletionInfo {
                                    completion_type:
                                        CompletionType::CallProperty(
                                            func.name,
                                        ),
                                    ident: name,
                                    position,
                                    uri: uri.clone(),
                                    imports: get_imports(
                                        uri, position, source,
                                    )?,
                                    package,
                                }));
                            }
                        }
                    }
                }

                match finder_node.node.as_ref() {
                    AstNode::BinaryExpr(be) => {
                        if let Expression::Identifier(left) =
                            be.left.clone()
                        {
                            let name = left.name;

                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Logical(
                                        be.operator.clone(),
                                    ),
                                ident: name,
                                position,
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, source,
                                )?,
                                package,
                            }));
                        }
                    }
                    AstNode::Identifier(ident) => {
                        let name = ident.name.clone();
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Generic,
                            ident: name,
                            position,
                            uri: uri.clone(),
                            imports: get_imports(
                                uri, position, source,
                            )?,
                            package,
                        }));
                    }
                    AstNode::BadExpr(expr) => {
                        let name = expr.text.clone();
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Bad,
                            ident: name,
                            position,
                            uri: uri.clone(),
                            imports: get_imports(
                                uri, position, source,
                            )?,
                            package,
                        }));
                    }
                    AstNode::MemberExpr(mbr) => {
                        if let Expression::Identifier(ident) =
                            &mbr.object
                        {
                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Generic,
                                ident: ident.name.clone(),
                                position,
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, source,
                                )?,
                                package,
                            }));
                        }
                    }
                    AstNode::CallExpr(c) => {
                        if let Some(Expression::Identifier(ident)) =
                            c.arguments.last()
                        {
                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Generic,
                                ident: ident.name.clone(),
                                position,
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, source,
                                )?,
                                package,
                            }));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(None)
    }
}

fn get_imports(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: String,
) -> Result<Vec<Import>, String> {
    let pkg = create_completion_package(uri, pos, contents)?;
    let walker = SemanticNode::Package(&pkg);
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
}

fn get_imports_removed(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: String,
) -> Result<Vec<Import>, String> {
    let pkg = create_completion_package_removed(uri, pos, contents)?;
    let walker = SemanticNode::Package(&pkg);
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
}

fn move_back(position: lsp::Position, count: u32) -> lsp::Position {
    lsp::Position {
        line: position.line,
        character: position.character - count,
    }
}

fn get_user_matches(
    info: CompletionInfo,
    contents: String,
) -> Result<Vec<lsp::CompletionItem>, Error> {
    let completables = get_user_completables(
        info.uri.clone(),
        info.position,
        contents.clone(),
    )?;

    let mut result: Vec<lsp::CompletionItem> = vec![];
    for x in completables {
        if x.matches(contents.clone(), info.clone()) {
            result.push(x.completion_item(info.clone()))
        }
    }

    Ok(result)
}

fn get_trigger(params: lsp::CompletionParams) -> Option<String> {
    if let Some(context) = params.context {
        context.trigger_character
    } else {
        None
    }
}

fn find_dot_completions(
    params: lsp::CompletionParams,
    contents: String,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri.clone();
    let pos = params.text_document_position.position;
    let info = CompletionInfo::create(params, contents.clone())?;

    if let Some(info) = info {
        let imports = info.imports.clone();

        let mut list = vec![];
        let name = info.ident.clone();
        get_specific_package_functions(&mut list, name, imports);

        let mut items = vec![];
        let obj_results = get_specific_object(
            info.ident.clone(),
            pos,
            uri,
            contents,
        )?;

        for completable in obj_results.into_iter() {
            items.push(completable.completion_item(info.clone()));
        }

        for item in list.into_iter() {
            items.push(item.completion_item(info.clone()));
        }

        return Ok(lsp::CompletionList {
            is_incomplete: false,
            items,
        });
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}
// XXX: sean (10 Aug 2021) - This function is a no-op, since the new server
// does not yet support completion callbacks
async fn find_arg_completions(
    params: lsp::CompletionParams,
    source: String,
) -> Result<lsp::CompletionList, Error> {
    let callbacks = crate::shared::Callbacks {
        buckets: None,
        measurements: None,
        tag_keys: None,
        tag_values: None,
    };
    let info = CompletionInfo::create(params.clone(), source)?;

    if let Some(info) = info {
        if info.ident == "bucket" {
            return get_bucket_completions(
                callbacks,
                get_trigger(params),
            )
            .await;
        }
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}

async fn get_bucket_completions(
    callbacks: crate::shared::Callbacks,
    trigger: Option<String>,
) -> Result<lsp::CompletionList, Error> {
    let buckets = callbacks.get_buckets().await;

    let items: Vec<lsp::CompletionItem> = match buckets {
        Ok(value) => value
            .into_iter()
            .map(|value| {
                new_string_arg_completion(value, trigger.clone())
            })
            .collect(),
        Err(err) => {
            warn!("Error in bucket callback: {}", err);
            vec![]
        }
    };

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items,
    })
}

fn new_string_arg_completion(
    value: String,
    trigger: Option<String>,
) -> lsp::CompletionItem {
    let trigger = trigger.unwrap_or_else(|| "".to_string());
    let insert_text = if trigger == "\"" {
        value
    } else {
        format!("\"{}\"", value)
    };

    lsp::CompletionItem {
        deprecated: None,
        commit_characters: None,
        detail: None,
        label: insert_text.clone(),
        additional_text_edits: None,
        filter_text: None,
        insert_text: Some(insert_text),
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: Some(lsp::InsertTextFormat::Snippet),
        text_edit: None,
        kind: Some(lsp::CompletionItemKind::Value),
        command: None,
        data: None,
        insert_text_mode: None,
        tags: None,
    }
}

fn get_user_completables(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: String,
) -> Result<Vec<Arc<dyn Completable>>, Error> {
    let pkg = create_completion_package(uri, pos, contents)?;
    let walker = SemanticNode::Package(&pkg);
    let mut visitor = CompletableFinderVisitor::new(pos);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).completables.clone());
    }

    Err(Error {
        msg: "failed to get completables".to_string(),
    })
}

fn get_stdlib_matches(
    name: String,
    info: CompletionInfo,
) -> Vec<lsp::CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib_completables();

    for c in completes.into_iter() {
        if c.matches(name.clone(), info.clone()) {
            matches.push(c.completion_item(info.clone()));
        }
    }

    matches
}

fn find_param_completions(
    trigger: Option<String>,
    params: lsp::CompletionParams,
    source: String,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let pkg: Package =
        parse_string(uri.to_string(), source.as_str()).into();
    let walker = Rc::new(AstNode::File(&pkg.files[0]));
    let visitor = CallFinderVisitor::new(move_back(position, 1));

    walk_rc(&visitor, walker);

    let state = visitor.state.borrow();
    let node = (*state).node.clone();
    let mut items: Vec<String> = vec![];

    if let Some(node) = node {
        if let AstNode::CallExpr(call) = node.as_ref() {
            let provided = get_provided_arguments(call);

            if let Expression::Identifier(ident) = call.callee.clone()
            {
                items.extend(get_function_params(
                    ident.name.clone(),
                    get_builtin_functions(),
                    provided.clone(),
                ));

                if let Ok(user_functions) = get_user_functions(
                    uri.clone(),
                    position,
                    source.clone(),
                ) {
                    items.extend(get_function_params(
                        ident.name,
                        user_functions,
                        provided.clone(),
                    ));
                }
            }
            if let Expression::Member(me) = call.callee.clone() {
                if let Expression::Identifier(ident) = me.object {
                    let package_functions =
                        get_package_functions(ident.name.clone());

                    let object_functions = get_object_functions(
                        uri, position, ident.name, source,
                    )?;

                    let key = match me.property {
                        PropertyKey::Identifier(i) => i.name,
                        PropertyKey::StringLit(l) => l.value,
                    };

                    items.extend(get_function_params(
                        key.clone(),
                        package_functions,
                        provided.clone(),
                    ));

                    items.extend(get_function_params(
                        key,
                        object_functions,
                        provided,
                    ));
                }
            }
        }
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: items
            .into_iter()
            .map(|x| new_param_completion(x, trigger.clone()))
            .collect(),
    })
}

fn get_specific_package_functions(
    list: &mut Vec<Box<dyn Completable>>,
    name: String,
    current_imports: Vec<Import>,
) {
    if let Some(env) = imports() {
        if let Some(import) =
            current_imports.into_iter().find(|x| x.alias == name)
        {
            for (key, val) in env.values {
                if *key == import.path {
                    walk_package(key.to_string(), list, val.expr);
                }
            }
        } else {
            for (key, val) in env.values {
                if let Some(package_name) =
                    get_package_name(key.to_string())
                {
                    if package_name == name {
                        walk_package(key.to_string(), list, val.expr);
                    }
                }
            }
        }
    }
}

fn get_specific_object(
    name: String,
    pos: lsp::Position,
    uri: lsp::Url,
    contents: String,
) -> Result<Vec<Arc<dyn Completable>>, Error> {
    let pkg = create_completion_package_removed(uri, pos, contents)?;
    let walker = SemanticNode::Package(&pkg);
    let mut visitor = CompletableObjectFinderVisitor::new(name);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok(state.completables.clone());
    }

    Ok(vec![])
}

fn get_provided_arguments(call: &flux::ast::CallExpr) -> Vec<String> {
    let mut provided = vec![];
    if let Some(Expression::Object(obj)) = call.arguments.first() {
        for prop in obj.properties.clone() {
            match prop.key {
                flux::ast::PropertyKey::Identifier(ident) => {
                    provided.push(ident.name)
                }
                flux::ast::PropertyKey::StringLit(lit) => {
                    provided.push(lit.value)
                }
            };
        }
    }

    provided
}

fn get_function_params(
    name: String,
    functions: Vec<Function>,
    provided: Vec<String>,
) -> Vec<String> {
    functions.into_iter().filter(|f| f.name == name).fold(
        vec![],
        |mut acc, f| {
            acc.extend(
                f.params
                    .into_iter()
                    .filter(|p| !provided.contains(p)),
            );
            acc
        },
    )
}

fn get_user_functions(
    uri: lsp::Url,
    pos: lsp::Position,
    source: String,
) -> Result<Vec<Function>, Error> {
    let pkg = create_completion_package(uri, pos, source)?;
    let walker = SemanticNode::Package(&pkg);
    let mut visitor = FunctionFinderVisitor::new(pos);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).functions.clone());
    }

    Err(Error {
        msg: "failed to get completables".to_string(),
    })
}

fn get_object_functions(
    uri: lsp::Url,
    pos: lsp::Position,
    object: String,
    contents: String,
) -> Result<Vec<Function>, Error> {
    let pkg = create_completion_package(uri, pos, contents)?;
    let walker = SemanticNode::Package(&pkg);
    let mut visitor = ObjectFunctionFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok(state
            .results
            .clone()
            .into_iter()
            .filter(|obj| obj.object == object)
            .map(|obj| obj.function)
            .collect());
    }

    Ok(vec![])
}

fn new_param_completion(
    name: String,
    trigger: Option<String>,
) -> lsp::CompletionItem {
    let insert_text = if let Some(trigger) = trigger {
        if trigger == "(" {
            format!("{}: ", name)
        } else {
            format!(" {}: ", name)
        }
    } else {
        format!("{}: ", name)
    };

    lsp::CompletionItem {
        deprecated: None,
        commit_characters: None,
        detail: None,
        label: name,
        additional_text_edits: None,
        filter_text: None,
        insert_text: Some(insert_text),
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: Some(lsp::InsertTextFormat::Snippet),
        text_edit: None,
        kind: Some(lsp::CompletionItemKind::Field),
        command: None,
        data: None,
        insert_text_mode: None,
        tags: None,
    }
}

fn walk_package(
    package: String,
    list: &mut Vec<Box<dyn Completable>>,
    t: MonoType,
) {
    if let MonoType::Record(record) = t {
        if let Record::Extension { head, tail } = record.as_ref() {
            let mut push_var_result = |name: &str, var_type| {
                list.push(Box::new(VarResult {
                    name: name.to_owned(),
                    var_type,
                    package: package.clone(),
                    package_name: get_package_name(package.clone()),
                }));
            };

            match &head.v {
                MonoType::Fun(f) => {
                    list.push(Box::new(FunctionResult {
                        name: head.k.clone(),
                        package: package.clone(),
                        signature: create_function_signature(
                            f
                        ),
                        required_args: get_argument_names(
                            &f.req,
                        ),
                        optional_args: get_argument_names(
                            &f.opt,
                        ),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Int => {
                    push_var_result(&head.k, VarType::Int)
                }
                MonoType::Float => {
                    push_var_result(&head.k, VarType::Float)
                }
                MonoType::Bool => {
                    push_var_result(&head.k, VarType::Bool)
                }
                MonoType::Arr(_) => {
                    push_var_result(&head.k, VarType::Array)
                }
                MonoType::Bytes => {
                    push_var_result(&head.k, VarType::Bytes)
                }
                MonoType::Duration => {
                    push_var_result(&head.k, VarType::Duration)
                }
                MonoType::Regexp => {
                    push_var_result(&head.k, VarType::Regexp)
                }
                MonoType::String => {
                    push_var_result(&head.k, VarType::String)
                }
                _ => {}
            }

            walk_package(package, list, tail.deref().clone());
        }
    }
}

trait Completable {
    fn completion_item(
        &self,
        info: CompletionInfo,
    ) -> lsp::CompletionItem;
    fn matches(&self, text: String, info: CompletionInfo) -> bool;
}

// Reports if the needle has a fuzzy match with the haystack.
//
// It is assumed that the haystack is the name of an identifier and the needle is a partial
// identifier.
fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    return haystack
        .to_lowercase()
        .contains(needle.to_lowercase().as_str());
}

impl Completable for PackageResult {
    fn completion_item(
        &self,
        info: CompletionInfo,
    ) -> lsp::CompletionItem {
        let imports = info.imports;
        let mut additional_text_edits = vec![];
        let mut insert_text = self.name.clone();

        if imports
            .clone()
            .into_iter()
            .map(|x| x.path)
            .any(|x| x == self.full_name)
        {
            let alias =
                find_alias_name(imports, self.name.clone(), 1);

            let new_text = if let Some(alias) = alias {
                insert_text = alias.clone();
                format!("import {} \"{}\"\n", alias, self.full_name)
            } else {
                format!("import \"{}\"\n", self.full_name)
            };

            let line = match info.package {
                Some(pi) => pi.position.line + 1,
                None => 0,
            };

            additional_text_edits.push(lsp::TextEdit {
                new_text,
                range: lsp::Range {
                    start: lsp::Position { character: 0, line },
                    end: lsp::Position { character: 0, line },
                },
            })
        } else {
            for import in imports {
                if self.full_name == import.path {
                    insert_text = import.alias;
                }
            }
        }

        lsp::CompletionItem {
            label: self.full_name.clone(),
            additional_text_edits: Some(additional_text_edits),
            commit_characters: None,
            deprecated: None,
            detail: Some("Package".to_string()),
            documentation: Some(lsp::Documentation::String(
                self.full_name.clone(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(insert_text),
            insert_text_format: Some(
                lsp::InsertTextFormat::PlainText,
            ),
            kind: Some(lsp::CompletionItemKind::Module),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: String, _info: CompletionInfo) -> bool {
        fuzzy_match(self.name.as_str(), text.as_str())
    }
}

impl Completable for FunctionResult {
    fn completion_item(
        &self,
        info: CompletionInfo,
    ) -> lsp::CompletionItem {
        let imports = info.imports;
        let mut additional_text_edits = vec![];

        let contains_pkg =
            imports.into_iter().any(|x| self.package == x.path);

        if !contains_pkg && self.package != PRELUDE_PACKAGE {
            additional_text_edits.push(lsp::TextEdit {
                new_text: format!("import \"{}\"\n", self.package),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                },
            })
        }

        lsp::CompletionItem {
            label: self.name.clone(),
            additional_text_edits: Some(additional_text_edits),
            commit_characters: None,
            deprecated: None,
            detail: Some(self.signature.clone()),
            documentation: None,
            filter_text: Some(self.name.clone()),
            insert_text: None,
            insert_text_format: Some(lsp::InsertTextFormat::Snippet),
            kind: Some(lsp::CompletionItemKind::Function),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: String, info: CompletionInfo) -> bool {
        let imports = info.imports;
        if self.package == PRELUDE_PACKAGE
            && fuzzy_match(self.name.as_str(), text.as_str())
        {
            return true;
        }

        if !imports
            .clone()
            .into_iter()
            .any(|x| self.package == x.path)
        {
            return false;
        }

        if text.ends_with('.') {
            let mtext = text[..text.len() - 1].to_string();
            return imports
                .into_iter()
                .any(|import| import.alias == mtext);
        }

        false
    }
}

impl Completable for CompletionVarResult {
    fn completion_item(
        &self,
        _info: CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} (self)", self.name),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some(self.detail()),
            documentation: Some(lsp::Documentation::String(
                "from self".to_string(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: Some(
                lsp::InsertTextFormat::PlainText,
            ),
            kind: Some(lsp::CompletionItemKind::Variable),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: String, _info: CompletionInfo) -> bool {
        fuzzy_match(self.name.as_str(), text.as_str())
    }
}

fn get_stdlib_completables() -> Vec<Box<dyn Completable>> {
    let mut list = vec![];

    get_packages(&mut list);
    get_builtins(&mut list);

    list
}

fn get_packages(list: &mut Vec<Box<dyn Completable>>) {
    if let Some(env) = imports() {
        for (key, _val) in env.values {
            add_package_result(key.to_string(), list);
        }
    }
}

fn get_builtins(list: &mut Vec<Box<dyn Completable>>) {
    if let Some(env) = prelude() {
        for (key, val) in env.values {
            let mut push_var_result = |var_type| {
                list.push(Box::new(VarResult {
                    name: key.to_string(),
                    package: PRELUDE_PACKAGE.to_string(),
                    package_name: None,
                    var_type,
                }));
            };
            match &val.expr {
                MonoType::Fun(f) => {
                    list.push(Box::new(FunctionResult {
                        package: PRELUDE_PACKAGE.to_string(),
                        package_name: None,
                        name: key.to_string(),
                        signature: create_function_signature(
                            &f,
                        ),
                        required_args: get_argument_names(&f.req),
                        optional_args: get_argument_names(&f.opt),
                    }))
                }
                MonoType::String => push_var_result(VarType::String),
                MonoType::Int => push_var_result(VarType::Int),
                MonoType::Float => push_var_result(VarType::Float),
                MonoType::Arr(_) => push_var_result(VarType::Array),
                MonoType::Bool => push_var_result(VarType::Bool),
                MonoType::Bytes => push_var_result(VarType::Bytes),
                MonoType::Duration => {
                    push_var_result(VarType::Duration)
                }
                MonoType::Uint => push_var_result(VarType::Uint),
                MonoType::Regexp => push_var_result(VarType::Regexp),
                MonoType::Time => push_var_result(VarType::Time),
                _ => {}
            }
        }
    }
}

fn find_alias_name(
    imports: Vec<Import>,
    name: String,
    iteration: i32,
) -> Option<String> {
    let first_iteration = iteration == 1;
    let pkg_name = if first_iteration {
        name.clone()
    } else {
        format!("{}{}", name, iteration)
    };

    for import in imports.clone() {
        if import.alias == pkg_name {
            return find_alias_name(imports, name, iteration + 1);
        }

        if let Some(initial_name) = import.initial_name {
            if initial_name == pkg_name && first_iteration {
                return find_alias_name(imports, name, iteration + 1);
            }
        }
    }

    if first_iteration {
        return None;
    }

    Some(format!("{}{}", name, iteration))
}

fn add_package_result(
    name: String,
    list: &mut Vec<Box<dyn Completable>>,
) {
    let package_name = get_package_name(name.clone());
    if let Some(package_name) = package_name {
        list.push(Box::new(PackageResult {
            name: package_name,
            full_name: name,
        }));
    }
}

impl PackageFinderVisitor {
    fn find_package(
        uri: lsp::Url,
        contents: String,
    ) -> Result<Option<PackageInfo>, String> {
        let package = create_ast_package(uri, contents)?;
        for file in package.files {
            let walker = Rc::new(AstNode::File(&file));
            let visitor = PackageFinderVisitor::default();

            walk_rc(&visitor, walker);

            let state = visitor.state.borrow();
            if let Some(info) = state.info.clone() {
                return Ok(Some(info));
            }
        }

        Ok(None)
    }
}

fn create_ast_package(
    uri: lsp::Url,
    source: String,
) -> Result<flux::ast::Package, String> {
    let mut pkg: Package =
        parse_string(uri.to_string(), source.as_str()).into();
    pkg.files.sort_by(|a, _b| {
        if a.name == uri.as_str() {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        }
    });

    Ok(pkg)
}

#[derive(Default)]
struct CompletableFinderState {
    completables: Vec<Arc<dyn Completable>>,
}

struct CompletableFinderVisitor {
    pos: lsp::Position,
    state: Arc<Mutex<CompletableFinderState>>,
}

impl<'a> SemanticVisitor<'a> for CompletableFinderVisitor {
    fn visit(&mut self, node: SemanticNode<'a>) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let loc = node.loc();

            if defined_after(loc, self.pos) {
                return true;
            }

            if let SemanticNode::ImportDeclaration(id) = node {
                if let Some(alias) = id.alias.clone() {
                    (*state).completables.push(Arc::new(
                        ImportAliasResult::new(
                            id.path.value.clone(),
                            alias.name.to_string(),
                        ),
                    ));
                }
            }

            if let SemanticNode::VariableAssgn(assgn) = node {
                let name = assgn.id.name.clone();
                if let Some(var_type) = get_var_type(&assgn.init) {
                    (*state).completables.push(Arc::new(
                        CompletionVarResult {
                            var_type,
                            name: name.to_string(),
                        },
                    ));
                }

                if let Some(fun) = create_function_result(
                    name.to_string(),
                    &assgn.init,
                ) {
                    (*state).completables.push(Arc::new(fun));
                }
            }

            if let SemanticNode::OptionStmt(opt) = node {
                if let flux::semantic::nodes::Assignment::Variable(
                    var_assign,
                ) = &opt.assignment
                {
                    let name = var_assign.id.name.clone();
                    if let Some(var_type) =
                        get_var_type(&var_assign.init)
                    {
                        (*state).completables.push(Arc::new(
                            CompletionVarResult {
                                name: name.to_string(),
                                var_type,
                            },
                        ));

                        return false;
                    }

                    if let Some(fun) = create_function_result(
                        name.to_string(),
                        &var_assign.init,
                    ) {
                        (*state).completables.push(Arc::new(fun));
                        return false;
                    }
                }
            }
        }

        true
    }
}

impl CompletableFinderVisitor {
    fn new(pos: lsp::Position) -> Self {
        CompletableFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableFinderState::default(),
            )),
            pos,
        }
    }
}

fn defined_after(loc: &SourceLocation, pos: lsp::Position) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return true;
    }

    false
}

#[derive(Clone)]
struct ImportAliasResult {
    path: String,
    alias: String,
}

impl ImportAliasResult {
    fn new(path: String, alias: String) -> Self {
        ImportAliasResult { path, alias }
    }
}

impl Completable for ImportAliasResult {
    fn completion_item(
        &self,
        _info: CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} (self)", self.alias),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some("Package".to_string()),
            documentation: Some(lsp::Documentation::String(format!(
                "from {}",
                self.path
            ))),
            filter_text: Some(self.alias.clone()),
            insert_text: Some(self.alias.clone()),
            insert_text_format: Some(lsp::InsertTextFormat::Snippet),
            kind: Some(lsp::CompletionItemKind::Module),
            preselect: None,
            sort_text: Some(self.alias.clone()),
            text_edit: None,

            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: String, _info: CompletionInfo) -> bool {
        fuzzy_match(self.alias.as_str(), text.as_str())
    }
}

fn get_var_type(
    expr: &SemanticExpression,
) -> Option<CompletionVarType> {
    if let Some(typ) =
        CompletionVarType::from_monotype(&expr.type_of())
    {
        return Some(typ);
    }

    match expr {
        SemanticExpression::Object(_) => {
            Some(CompletionVarType::Object)
        }
        SemanticExpression::Call(c) => {
            let result_type = follow_function_pipes(c);

            match CompletionVarType::from_monotype(result_type) {
                Some(typ) => Some(typ),
                None => match result_type {
                    MonoType::Record(_) => {
                        Some(CompletionVarType::Record)
                    }
                    _ => None,
                },
            }
        }
        _ => None,
    }
}

fn create_function_result(
    name: String,
    expr: &SemanticExpression,
) -> Option<UserFunctionResult> {
    if let SemanticExpression::Function(f) = expr {
        if let MonoType::Fun(fun) = &f.typ {
            return Some(UserFunctionResult {
                name,
                package: "self".to_string(),
                package_name: Some("self".to_string()),
                optional_args: get_argument_names(&fun.opt),
                required_args: get_argument_names(&fun.req),
                signature: create_function_signature(fun),
            });
        }
    }

    None
}

fn follow_function_pipes(c: &CallExpr) -> &MonoType {
    if let Some(SemanticExpression::Call(call)) = &c.pipe {
        return follow_function_pipes(call);
    }

    &c.typ
}

#[derive(Default)]
struct CompletableObjectFinderState {
    completables: Vec<Arc<dyn Completable>>,
}

struct CompletableObjectFinderVisitor {
    name: String,
    state: Arc<Mutex<CompletableObjectFinderState>>,
}

impl CompletableObjectFinderVisitor {
    fn new(name: String) -> Self {
        CompletableObjectFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableObjectFinderState::default(),
            )),
            name,
        }
    }
}

impl<'a> SemanticVisitor<'a> for CompletableObjectFinderVisitor {
    fn visit(&mut self, node: SemanticNode<'a>) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let name = self.name.clone();

            if let SemanticNode::ObjectExpr(obj) = node {
                if let Some(ident) = &obj.with {
                    if name == *ident.name {
                        for prop in obj.properties.clone() {
                            let name = prop.key.name;
                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    CompletionVarResult {
                                        var_type,
                                        name: name.to_string(),
                                    },
                                ));
                            }
                            if let Some(fun) = create_function_result(
                                name.to_string(),
                                &prop.value,
                            ) {
                                (*state)
                                    .completables
                                    .push(Arc::new(fun));
                            }
                        }
                    }
                }
            }

            if let SemanticNode::VariableAssgn(assign) = node {
                if *assign.id.name == name {
                    if let SemanticExpression::Object(obj) =
                        &assign.init
                    {
                        for prop in obj.properties.clone() {
                            let name = prop.key.name;

                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    CompletionVarResult {
                                        var_type,
                                        name: name.to_string(),
                                    },
                                ));
                            }

                            if let Some(fun) = create_function_result(
                                name.to_string(),
                                &prop.value,
                            ) {
                                (*state)
                                    .completables
                                    .push(Arc::new(fun));
                            }
                        }

                        return false;
                    }
                }
            }

            if let SemanticNode::OptionStmt(opt) = node {
                if let flux::semantic::nodes::Assignment::Variable(
                    assign,
                ) = opt.assignment.clone()
                {
                    if *assign.id.name == name {
                        if let SemanticExpression::Object(obj) =
                            assign.init
                        {
                            for prop in obj.properties.clone() {
                                let name = prop.key.name;
                                if let Some(var_type) =
                                    get_var_type(&prop.value)
                                {
                                    (*state).completables.push(
                                        Arc::new(
                                            CompletionVarResult {
                                                var_type,
                                                name: name
                                                    .to_string(),
                                            },
                                        ),
                                    );
                                }
                                if let Some(fun) =
                                    create_function_result(
                                        name.to_string(),
                                        &prop.value,
                                    )
                                {
                                    (*state)
                                        .completables
                                        .push(Arc::new(fun));
                                }
                            }
                            return false;
                        }
                    }
                }
            }
        }

        true
    }
}

#[derive(Clone)]
struct CompletionVarResult {
    name: String,
    var_type: CompletionVarType,
}

#[derive(Clone)]
enum CompletionVarType {
    Int,
    String,
    Array,
    Float,
    Bool,
    Duration,
    Object,
    Regexp,
    Record,
    Uint,
    Time,
}

#[derive(Clone)]
struct VarResult {
    name: String,
    var_type: VarType,
    package: String,
    package_name: Option<String>,
}

impl VarResult {
    fn detail(&self) -> String {
        match self.var_type {
            VarType::Array => "Array".to_string(),
            VarType::Bool => "Boolean".to_string(),
            VarType::Bytes => "Bytes".to_string(),
            VarType::Duration => "Duration".to_string(),
            VarType::Float => "Float".to_string(),
            VarType::Int => "Integer".to_string(),
            VarType::Regexp => "Regular Expression".to_string(),
            VarType::String => "String".to_string(),
            VarType::Uint => "Uint".to_string(),
            VarType::Time => "Time".to_string(),
        }
    }
}

impl Completable for VarResult {
    fn completion_item(
        &self,
        _info: CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} ({})", self.name, self.package),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some(self.detail()),
            documentation: Some(lsp::Documentation::String(format!(
                "from {}",
                self.package
            ))),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: Some(
                lsp::InsertTextFormat::PlainText,
            ),
            kind: Some(lsp::CompletionItemKind::Variable),
            preselect: None,
            sort_text: Some(format!(
                "{} {}",
                self.name, self.package
            )),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: String, info: CompletionInfo) -> bool {
        let imports = info.imports;
        if self.package == PRELUDE_PACKAGE
            && fuzzy_match(self.name.as_str(), text.as_str())
        {
            return true;
        }

        if !imports.into_iter().any(|x| self.package == x.path) {
            return false;
        }

        if text.ends_with('.') {
            let mtext = text[..text.len() - 1].to_string();
            return Some(mtext) == self.package_name;
        }

        false
    }
}

impl CompletionVarResult {
    fn detail(&self) -> String {
        match self.var_type {
            CompletionVarType::Array => "Array".to_string(),
            CompletionVarType::Bool => "Boolean".to_string(),
            CompletionVarType::Duration => "Duration".to_string(),
            CompletionVarType::Float => "Float".to_string(),
            CompletionVarType::Int => "Integer".to_string(),
            CompletionVarType::Object => "Object".to_string(),
            CompletionVarType::Regexp => {
                "Regular Expression".to_string()
            }
            CompletionVarType::String => "String".to_string(),
            CompletionVarType::Record => "Record".to_string(),
            CompletionVarType::Time => "Time".to_string(),
            CompletionVarType::Uint => "Unsigned Integer".to_string(),
        }
    }
}

impl CompletionVarType {
    pub fn from_monotype(typ: &MonoType) -> Option<Self> {
        Some(match typ {
            MonoType::Duration => CompletionVarType::Duration,
            MonoType::Int => CompletionVarType::Int,
            MonoType::Bool => CompletionVarType::Bool,
            MonoType::Float => CompletionVarType::Float,
            MonoType::String => CompletionVarType::String,
            MonoType::Arr(_) => CompletionVarType::Array,
            MonoType::Regexp => CompletionVarType::Regexp,
            MonoType::Uint => CompletionVarType::Uint,
            MonoType::Time => CompletionVarType::Time,
            _ => return None,
        })
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
    let mut visitor = SemanticNodeFinderVisitor::new(position);

    flux::semantic::walk::walk(&mut visitor, node);

    let state = visitor.state.borrow();

    result.node = (*state).node.clone();
    result.path = (*state).path.clone();

    result
}
// fn create_diagnostics_notification(
//     uri: lsp::Url,
//     diagnostics: Vec<lsp::Diagnostic>,
// ) -> Notification<lsp::PublishDiagnosticsParams> {
//     let method = String::from("textDocument/publishDiagnostics");
//     let params = lsp::PublishDiagnosticsParams {
//         uri,
//         diagnostics,
//         version: None,
//     };
//     Notification { method, params }
// }
// #[derive(Serialize, Deserialize)]
// struct Notification<T> {
//     method: String,
//     params: T,
// }
//
// impl<T> Notification<T>
// where
//     T: Serialize,
// {
//     fn to_json(&self) -> Result<String, String> {
//         match serde_json::to_string(self) {
//             Ok(s) => Ok(s),
//             Err(_) => Err(String::from(
//                 "Failed to serialize initialize response",
//             )),
//         }
//     }
// }
#[derive(Clone)]
struct FunctionResult {
    name: String,
    package: String,
    #[allow(dead_code)]
    package_name: Option<String>,
    #[allow(dead_code)]
    required_args: Vec<String>,
    #[allow(dead_code)]
    optional_args: Vec<String>,
    signature: String,
}
#[derive(Clone)]
struct PackageResult {
    name: String,
    full_name: String,
}
#[derive(Clone)]
enum VarType {
    Int,
    String,
    Array,
    Float,
    Bool,
    Bytes,
    Duration,
    Regexp,
    Uint,
    Time,
}

#[derive(Debug, Clone, PartialEq)]
struct Property {
    k: String,
    v: String,
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.k, self.v)
    }
}

fn create_completion_package_removed(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: String,
) -> Result<flux::semantic::nodes::Package, String> {
    let mut file = parse_string("".to_string(), contents.as_str());

    file.imports = file
        .imports
        .into_iter()
        .filter(|x| valid_node(&x.base, pos))
        .collect();

    file.body = file
        .body
        .into_iter()
        .filter(|x| valid_node(x.base(), pos))
        .collect();

    let mut pkg = create_ast_package(uri.clone(), contents)?;

    pkg.files = pkg
        .files
        .into_iter()
        .map(|curr| {
            if curr.name == uri.as_str() {
                file.clone()
            } else {
                curr
            }
        })
        .collect();

    match analyze(pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("ERROR IS HERE {}", e)),
    }
}

fn create_completion_package(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: String,
) -> Result<flux::semantic::nodes::Package, String> {
    create_filtered_package(uri, contents, |x| {
        valid_node(x.base(), pos)
    })
}

fn valid_node(
    node: &flux::ast::BaseNode,
    position: lsp::Position,
) -> bool {
    !is_in_node(position, node)
}

fn create_filtered_package<F>(
    uri: lsp::Url,
    contents: String,
    mut filter: F,
) -> Result<flux::semantic::nodes::Package, String>
where
    F: FnMut(&flux::ast::Statement) -> bool,
{
    let mut ast_pkg = create_ast_package(uri.clone(), contents)?;

    ast_pkg.files = ast_pkg
        .files
        .into_iter()
        .map(|mut file| {
            if file.name == uri.as_str() {
                file.body = file
                    .body
                    .into_iter()
                    .filter(|x| filter(x))
                    .collect();
            }

            file
        })
        .collect();

    match analyze(ast_pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("{}", e)),
    }
}

#[derive(Clone)]
pub struct UserFunctionResult {
    pub name: String,
    pub package: String,
    pub package_name: Option<String>,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
    pub signature: String,
}

impl UserFunctionResult {
    fn insert_text(&self) -> String {
        let mut insert_text = format!("{}(", self.name);

        for (index, arg) in self.required_args.iter().enumerate() {
            insert_text +=
                (format!("{}: ${}", arg, index + 1)).as_str();

            if index != self.required_args.len() - 1 {
                insert_text += ", ";
            }
        }

        if self.required_args.is_empty()
            && !self.optional_args.is_empty()
        {
            insert_text += "$1";
        }

        insert_text += ")$0";

        insert_text
    }
}

impl Completable for UserFunctionResult {
    fn completion_item(
        &self,
        _info: CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} (self)", self.name),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some(self.signature.clone()),
            documentation: Some(lsp::Documentation::String(
                "from self".to_string(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.insert_text()),
            insert_text_format: Some(lsp::InsertTextFormat::Snippet),
            kind: Some(lsp::CompletionItemKind::Function),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: String, _info: CompletionInfo) -> bool {
        fuzzy_match(self.name.as_str(), text.as_str())
    }
}
