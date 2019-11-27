use crate::protocol::notifications::{
    create_diagnostics_notification, Notification,
    PublishDiagnosticsParams,
};
use crate::protocol::properties::ContentChange;
use crate::protocol::requests::{Request, TextDocumentChangeParams};
use crate::utils::{self, create_file_node_from_text};

use flux::ast::{check, walk};

pub fn parse_change_request(
    data: String,
) -> Result<Request<TextDocumentChangeParams>, String> {
    let request: Request<TextDocumentChangeParams> =
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
    let file = create_file_node_from_text(uri.clone(), contents);
    let walker = walk::Node::File(&file);
    let errors = check::check(walker);
    let diagnostics = utils::map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri.clone(), diagnostics) {
        Ok(msg) => Ok(msg),
        Err(e) => Err(format!("Failed to create diagnostic: {}", e)),
    }
}
