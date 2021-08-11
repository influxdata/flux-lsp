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

// Url::to_file_path doesn't exist in wasm-unknown-unknown, for kinda
// obvious reasons. Ignore these tests when executing against that target.
#[cfg(all(test, not(target_arch = "wasm32"), not(feature = "lsp2")))]
mod tests;

pub use router::Router;

use crate::cache::Cache;
use crate::protocol::PolymorphicRequest;
use crate::shared::RequestContext;
use crate::visitors::semantic::NodeFinderVisitor;

use std::rc::Rc;

use async_trait::async_trait;

use lsp_types as lsp;

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
