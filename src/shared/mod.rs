use crate::cache;
use crate::protocol::notifications::{
    create_diagnostics_notification, Notification,
    PublishDiagnosticsParams,
};
use crate::protocol::properties::ContentChange;
use crate::protocol::requests::{
    Request, TextDocumentChangeParams, TextDocumentParams,
    TextDocumentSaveParams,
};
use crate::utils;

pub mod callbacks;
pub mod signatures;

use combinations::Combinations;

#[derive(Clone)]
pub struct RequestContext {
    pub support_multiple_files: bool,
    pub callbacks: callbacks::Callbacks,
}

impl RequestContext {
    pub fn new(
        callbacks: callbacks::Callbacks,
        support_multiple_files: bool,
    ) -> Self {
        RequestContext {
            callbacks,
            support_multiple_files,
        }
    }
}

pub fn all_combos<T>(l: Vec<T>) -> Vec<Vec<T>>
where
    T: std::cmp::Ord + Clone,
{
    let mut result = vec![];
    let length = l.len();

    for i in 1..length {
        let c: Vec<Vec<T>> =
            Combinations::new(l.clone(), i).collect();
        result.extend(c);
    }

    result.push(l);

    result
}

pub fn parse_change_request(
    data: String,
) -> Result<Request<TextDocumentChangeParams>, String> {
    let request: Request<TextDocumentChangeParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

pub fn parse_save_request(
    data: String,
) -> Result<Request<TextDocumentSaveParams>, String> {
    let request: Request<TextDocumentSaveParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

pub fn parse_open_request(
    data: String,
) -> Result<Request<TextDocumentParams>, String> {
    let request: Request<TextDocumentParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

pub fn parse_close_request(
    data: String,
) -> Result<Request<TextDocumentParams>, String> {
    let request: Request<TextDocumentParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

pub fn apply_changes(
    original: String,
    changes: Vec<ContentChange>,
) -> String {
    for change in changes {
        if change.range.is_none() {
            return change.text;
        }
    }

    original
}

pub fn create_ast_package(
    uri: String,
    ctx: RequestContext,
) -> Result<flux::ast::Package, String> {
    let values =
        cache::get_package(uri.clone(), ctx.support_multiple_files)?;

    let pkgs = values
        .into_iter()
        .map(|v: cache::CacheValue| {
            utils::create_file_node_from_text(
                v.uri.clone(),
                v.contents,
            )
        })
        .collect::<Vec<flux::ast::Package>>();

    let pkg = pkgs.into_iter().fold(
        None,
        |acc: Option<flux::ast::Package>, pkg| {
            if let Some(mut p) = acc {
                let mut files = pkg.files;
                p.files.append(&mut files);
                return Some(p);
            }

            Some(pkg)
        },
    );

    if let Some(mut pkg) = pkg {
        let mut files = pkg.files;
        files.sort_by(|a, _b| {
            if a.name == uri.clone() {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        });
        pkg.files = files;

        return Ok(pkg);
    }

    Err("Failed to create package".to_string())
}

pub fn create_diagnoistics(
    uri: String,
    ctx: RequestContext,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let package = create_ast_package(uri.clone(), ctx)?;
    let walker = flux::ast::walk::Node::Package(&package);
    let errors = flux::ast::check::check(walker);
    let diagnostics = utils::map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri, diagnostics) {
        Ok(msg) => Ok(msg),
        Err(e) => Err(format!("Failed to create diagnostic: {}", e)),
    }
}

pub fn handle_close(data: String) -> Result<Option<String>, String> {
    let request = parse_close_request(data)?;

    if let Some(params) = request.params {
        let uri = params.text_document.uri;

        cache::remove(uri)?;

        return Ok(None);
    }

    Err("invalid textDocument/didClose request".to_string())
}

pub fn handle_open(
    data: String,
    ctx: RequestContext,
) -> Result<Option<String>, String> {
    let request = parse_open_request(data)?;

    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = params.text_document.text;

        cache::set(uri.clone(), version, text)?;
        let msg = create_diagnoistics(uri, ctx)?;

        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didOpen request".to_string())
}

pub fn handle_change(
    data: String,
    ctx: RequestContext,
) -> Result<Option<String>, String> {
    let request = parse_change_request(data)?;
    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let changes = params.content_changes;
        let version = params.text_document.version;

        let cv = cache::get(uri.clone())?;
        let text = apply_changes(cv.contents, changes);

        cache::set(uri.clone(), version, text)?;

        let msg = create_diagnoistics(uri, ctx)?;
        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didChange request".to_string())
}

pub fn handle_save(
    data: String,
    ctx: RequestContext,
) -> Result<Option<String>, String> {
    let request = parse_save_request(data)?;
    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let msg = create_diagnoistics(uri, ctx)?;
        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didSave request".to_string())
}
