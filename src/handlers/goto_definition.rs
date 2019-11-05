use std::rc::Rc;

use crate::handlers::RequestHandler;
use crate::structs::{
    Location, PolymorphicRequest, Position, Range, Request, Response,
    TextDocumentPositionParams,
};
use crate::utils;
use crate::visitors::{DefinitionFinderVisitor, NodeFinderVisitor};

use flux::ast::walk;

fn ident_to_location(
    uri: String,
    i: flux::ast::Identifier,
) -> Location {
    let start = Position {
        line: i.base.location.start.line - 1,
        character: i.base.location.start.column - 1,
    };

    let end = Position {
        line: i.base.location.end.line - 1,
        character: i.base.location.end.column - 1,
    };

    let range = Range { start, end };
    let uri = uri.clone();

    Location { uri, range }
}

fn find_scoped_definition<'a>(
    uri: String,
    ident: &flux::ast::Identifier,
    path: Vec<Rc<walk::Node<'a>>>,
) -> Option<Location> {
    let path_iter = path.iter().rev();
    for n in path_iter {
        match n.as_ref() {
            walk::Node::FunctionExpr(_)
            | walk::Node::Package(_)
            | walk::Node::File(_) => {
                if let walk::Node::FunctionExpr(f) = n.as_ref() {
                    for param in f.params.clone() {
                        if let flux::ast::PropertyKey::Identifier(i) =
                            param.key
                        {
                            if i.name != ident.name {
                                continue;
                            }
                            let loc =
                                ident_to_location(uri.clone(), i);
                            return Some(loc);
                        }
                    }
                }

                let dvisitor: DefinitionFinderVisitor =
                    DefinitionFinderVisitor::new(ident.name.clone());

                walk::walk_rc(&dvisitor, n.clone());

                let state = dvisitor.state.borrow();

                if let Some(node) = state.node.clone() {
                    let loc = utils::map_node_to_location(uri, node);
                    return Some(loc);
                }
            }
            _ => (),
        }
    }
    return None;
}

pub struct GotoDefinitionHandler {}

impl GotoDefinitionHandler {
    pub fn new() -> GotoDefinitionHandler {
        GotoDefinitionHandler {}
    }
}

impl RequestHandler for GotoDefinitionHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let mut result: Option<Location> = None;

        let request: Request<TextDocumentPositionParams> =
            Request::from_json(prequest.data.as_str())?;

        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let file = utils::create_file_node(uri.clone())?;
            let walker = Rc::new(walk::Node::File(&file));

            let node_finder = NodeFinderVisitor::new(params.position);

            walk::walk_rc(&node_finder, walker);

            let state = node_finder.state.borrow();
            let node = (*state).node.clone();
            let path = (*state).path.clone();

            if let Some(node) = node {
                if let walk::Node::Identifier(ident) = node.as_ref() {
                    result = find_scoped_definition(
                        uri.clone(),
                        ident,
                        path,
                    );
                }
            }

            let response = Response {
                result,
                id: prequest.base_request.id,
                jsonrpc: "2.0".to_string(),
            };

            return response.to_json();
        }

        Err("invalid textDocument/definition request".to_string())
    }
}
