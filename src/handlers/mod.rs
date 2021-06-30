pub mod completion;
pub mod completion_resolve;
pub mod document_change;
pub mod document_close;
pub mod document_formatting;
pub mod document_open;
pub mod document_save;
pub mod document_symbol;
pub mod folding;
pub mod goto_definition;
pub mod initialize;
pub mod references;
pub mod rename;
pub mod router;
pub mod shutdown;
pub mod signature_help;

#[cfg(test)]
mod tests;

pub use router::Router;

use crate::cache::Cache;
use crate::protocol::notifications::{
    create_diagnostics_notification, Notification,
    PublishDiagnosticsParams,
};
use crate::protocol::requests::PolymorphicRequest;
use crate::shared::conversion::map_errors_to_diagnostics;
use crate::shared::RequestContext;
use crate::visitors::semantic::NodeFinderVisitor;

use std::rc::Rc;

use async_trait::async_trait;
use flux::ast::{check, walk};

use lspower::lsp;

#[derive(Debug)]
pub struct Error {
    pub msg: String,
}
impl From<String> for Error {
    fn from(s: String) -> Error {
        Error { msg: s }
    }
}

#[async_trait]
pub trait RequestHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error>;
}

pub fn create_diagnostics(
    uri: lsp::Url,
    file: flux::ast::File,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let walker = walk::Node::File(&file);

    let errors = check::check(walker);
    let diagnostics = map_errors_to_diagnostics(errors);

    Ok(create_diagnostics_notification(uri, diagnostics))
}

#[derive(Default, Clone)]
pub struct NodeFinderResult<'a> {
    pub node: Option<Rc<flux::semantic::walk::Node<'a>>>,
    pub path: Vec<Rc<flux::semantic::walk::Node<'a>>>,
}

pub fn find_node(
    node: flux::semantic::walk::Node<'_>,
    position: lsp::Position,
) -> NodeFinderResult<'_> {
    let mut result = NodeFinderResult::default();
    let mut visitor = NodeFinderVisitor::new(position);

    flux::semantic::walk::walk(&mut visitor, Rc::new(node));

    let state = visitor.state.borrow();

    result.node = (*state).node.clone();
    result.path = (*state).path.clone();

    result
}
