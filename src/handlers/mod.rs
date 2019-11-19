pub mod document_change;
pub mod document_close;
pub mod document_open;
pub mod document_save;
pub mod document_symbol;
pub mod folding;
pub mod goto_definition;
pub mod initialize;
pub mod references;
pub mod rename;
pub mod shutdown;

use crate::protocol::notifications::{
    create_diagnostics_notification, Notification,
    PublishDiagnosticsParams,
};
use crate::protocol::properties::Position;
use crate::protocol::requests::PolymorphicRequest;
use crate::utils;
use crate::visitors::semantic::NodeFinderVisitor;

use std::rc::Rc;

use flux::ast::{check, walk};

pub trait RequestHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String>;
}

pub fn create_diagnostics(
    uri: String,
    file: flux::ast::File,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let walker = walk::Node::File(&file);

    let errors = check::check(walker);
    let diagnostics = utils::map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri.clone(), diagnostics) {
        Ok(msg) => Ok(msg),
        Err(e) => Err(format!("Failed to create diagnostic: {}", e)),
    }
}

#[derive(Default, Clone)]
pub struct NodeFinderResult<'a> {
    node: Option<Rc<flux::semantic::walk::Node<'a>>>,
    path: Vec<Rc<flux::semantic::walk::Node<'a>>>,
}

pub fn find_node(
    node: flux::semantic::walk::Node<'_>,
    position: Position,
) -> NodeFinderResult<'_> {
    let mut result = NodeFinderResult::default();
    let mut visitor = NodeFinderVisitor::new(position);

    flux::semantic::walk::walk(&mut visitor, Rc::new(node));

    let state = visitor.state.borrow();

    result.node = (*state).node.clone();
    result.path = (*state).path.clone();

    result
}
