pub mod document_change;
pub mod document_open;
pub mod goto_definition;
pub mod initialize;
pub mod references;
pub mod shutdown;

use crate::structs::{
    create_diagnostics_notification, Notification,
    PolymorphicRequest, PublishDiagnosticsParams,
};
use crate::utils;

use flux::ast::{check, walk};

pub trait RequestHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String>;
}

pub fn create_file_diagnostics(
    uri: String,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let file = utils::create_file_node(uri.clone())?;
    let walker = walk::Node::File(&file);

    let errors = check::check(walker);
    let diagnostics = utils::map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri.clone(), diagnostics) {
        Ok(msg) => return Ok(msg),
        Err(e) => {
            return Err(format!("Failed to create diagnostic: {}", e))
        }
    };
}
