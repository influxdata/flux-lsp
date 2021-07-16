use std::rc::Rc;

use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request, Response};
use crate::shared::conversion::map_node_to_location;
use crate::visitors::semantic::utils;
use crate::visitors::semantic::{
    DefinitionFinderVisitor, NodeFinderVisitor,
};

use flux::semantic::walk::{self, Node};

use lsp_types as lsp;

fn ident_to_location(
    uri: lsp::Url,
    node: Rc<Node<'_>>,
) -> lsp::Location {
    let start = lsp::Position {
        line: node.loc().start.line - 1,
        character: node.loc().start.column - 1,
    };

    let end = lsp::Position {
        line: node.loc().end.line - 1,
        character: node.loc().end.column - 1,
    };

    let range = lsp::Range { start, end };

    lsp::Location { uri, range }
}

fn find_scoped_definition(
    uri: lsp::Url,
    ident_name: String,
    path: Vec<Rc<Node>>,
) -> Option<lsp::Location> {
    let path_iter = path.iter().rev();
    for n in path_iter {
        match n.as_ref() {
            walk::Node::FunctionExpr(_)
            | walk::Node::Package(_)
            | walk::Node::File(_) => {
                if let walk::Node::FunctionExpr(f) = n.as_ref() {
                    for param in f.params.clone() {
                        let name = param.key.name;
                        if name != ident_name {
                            continue;
                        }
                        let loc =
                            ident_to_location(uri, (*n).clone());
                        return Some(loc);
                    }
                }

                let mut dvisitor: DefinitionFinderVisitor =
                    DefinitionFinderVisitor::new(ident_name.clone());

                walk::walk(&mut dvisitor, n.clone());

                let state = dvisitor.state.borrow();

                if let Some(node) = state.node.clone() {
                    let loc = map_node_to_location(uri, node);
                    return Some(loc);
                }
            }
            _ => (),
        }
    }
    None
}

#[derive(Default)]
pub struct GotoDefinitionHandler {}

#[async_trait::async_trait]
impl RequestHandler for GotoDefinitionHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        let mut result: Option<lsp::Location> = None;

        let request: Request<lsp::TextDocumentPositionParams> =
            Request::from_json(prequest.data.as_str())?;

        if let Some(params) = request.params {
            let uri =
                lsp::Url::parse(params.text_document.uri.as_str())
                    .unwrap();
            let pkg =
                utils::create_semantic_package(uri.clone(), cache)?;
            let walker = Rc::new(walk::Node::Package(&pkg));

            let mut node_finder =
                NodeFinderVisitor::new(params.position);

            walk::walk(&mut node_finder, walker);

            let state = node_finder.state.borrow();
            let node = (*state).node.clone();
            let path = (*state).path.clone();

            if let Some(node) = node {
                let name = match node.as_ref() {
                    Node::Identifier(ident) => {
                        Some(ident.name.clone())
                    }
                    Node::IdentifierExpr(ident) => {
                        Some(ident.name.clone())
                    }
                    _ => None,
                };

                if let Some(name) = name {
                    result = find_scoped_definition(
                        lsp::Url::parse(&uri.to_string()).unwrap(),
                        name,
                        path,
                    );
                }
            }

            let id = prequest.base_request.id;
            let response = Response::new(id, result);
            let json = response.to_json()?;

            return Ok(Some(json));
        }

        Err(Error {
            msg: "invalid textDocument/definition request"
                .to_string(),
        })
    }
}
