use crate::cache;
use crate::protocol::notifications::{
    create_diagnostics_notification, Notification,
    PublishDiagnosticsParams,
};
use crate::protocol::properties::ContentChange;
use crate::protocol::properties::Position;
use crate::protocol::requests::{
    Request, TextDocumentChangeParams, TextDocumentParams,
    TextDocumentSaveParams,
};
use crate::utils;
use crate::visitors::ast::contains_line_ref;
use flux::ast::{
    walk::{create_visitor, walk, Node},
    CallExpr, Expression, PropertyKey,
};
use flux::parser::Parser;

pub mod callbacks;
pub mod signatures;

use combinations::Combinations;

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
}

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

pub fn find_ident_from_closest(src: &str, pos: &Position) -> String {
    let file = Parser::new(src).parse_file("".to_string());
    let mut result: String = "".to_owned();
    let mut min: Option<u32> = None;
    walk(
        &create_visitor(&mut |n| {
            if !contains_line_ref(n.as_ref(), &pos) {
                return; //skip
            }
            match n.as_ref() {
                Node::Identifier(ident) => {
                    if pos.character
                        < ident.base.location.end.column - 1
                        || ident.name.is_empty()
                    {
                        return; // pos is before the ident or empty ident
                    }
                    let distance = pos.character + 1
                        - ident.base.location.end.column;
                    match min {
                        Some(mm) => {
                            if mm > distance {
                                min = Some(distance);
                                result = ident.name.clone();
                            }
                        }
                        None => {
                            min = Some(distance);
                            result = ident.name.clone();
                        }
                    }
                }
                &_ => {}
            }
        }),
        Node::File(&file),
    );

    result
}

pub fn get_bucket(src: &str) -> String {
    let file = Parser::new(src).parse_file("".to_string());
    let mut result: Option<String> = None;
    walk(
        &create_visitor(&mut |n| {
            if result.is_some() {
                return;
            }

            match n.as_ref() {
                Node::CallExpr(exp) => {
                    result = Some(
                        get_bucket_from_call_expr(exp).to_owned(),
                    );
                }
                &_ => {}
            }
        }),
        Node::File(&file),
    );
    result.unwrap_or_default()
}

fn get_bucket_from_call_expr<'a>(exp: &&'a CallExpr) -> &'a str {
    // from(bucket:"bucket_name")
    // v1.measurementTagKeys(bucket:"buck3", measurement:)
    if exp.arguments.is_empty() {
        return "";
    }
    let args = exp.arguments.get(0);
    if args.is_none() {
        return "";
    }
    let good_args = Node::from_expr(args.unwrap());
    if let Node::ObjectExpr(ob) = good_args {
        for prop in &ob.properties {
            if let PropertyKey::Identifier(k) = &prop.key {
                if k.name == "bucket" {
                    if let Some(pv) = &prop.value {
                        if let Expression::StringLit(pp) = pv {
                            return &pp.value;
                        }
                    }
                }
            }
        }
    }
    ""
}

#[cfg(test)]
pub mod tests;
