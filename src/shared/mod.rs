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
use crate::visitors::ast;

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

pub fn create_diagnoistics(
    uri: String,
    contents: String,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let errors = ast::check_source(uri.clone(), contents);
    let diagnostics = utils::map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri.clone(), diagnostics) {
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

pub fn handle_open(data: String) -> Result<Option<String>, String> {
    let request = parse_open_request(data)?;

    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = params.text_document.text;

        cache::set(uri.clone(), version, text.clone())?;
        let msg = create_diagnoistics(uri.clone(), text)?;

        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didOpen request".to_string())
}

pub fn handle_change(data: String) -> Result<Option<String>, String> {
    let request = parse_change_request(data)?;
    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let changes = params.content_changes;
        let version = params.text_document.version;

        let cv = cache::get(uri.clone())?;
        let text = apply_changes(cv.contents, changes);

        cache::set(uri.clone(), version, text.clone())?;

        let msg = create_diagnoistics(uri.clone(), text.clone())?;
        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didChange request".to_string())
}

pub fn handle_save(data: String) -> Result<Option<String>, String> {
    let request = parse_save_request(data)?;
    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let cv = cache::get(uri.clone())?;
        let msg = create_diagnoistics(uri, cv.contents)?;
        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didSave request".to_string())
}
