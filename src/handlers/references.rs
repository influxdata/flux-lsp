use crate::handlers::find_node;
use crate::handlers::RequestHandler;
use crate::structs::{
    Location, PolymorphicRequest, Position, ReferenceParams, Request,
    Response,
};
use crate::utils;
use crate::visitors::{DefinitionFinderVisitor, IdentFinderVisitor};

use std::rc::Rc;

use flux::ast::walk;

fn function_defines(
    ident: &flux::ast::Identifier,
    f: &flux::ast::FunctionExpr,
) -> bool {
    for param in f.params.clone() {
        if let flux::ast::PropertyKey::Identifier(i) = param.key {
            if i.name == ident.name {
                return true;
            }
        }
    }

    false
}

fn is_scope<'a>(
    ident: &flux::ast::Identifier,
    n: Rc<walk::Node<'a>>,
) -> bool {
    let dvisitor: DefinitionFinderVisitor =
        DefinitionFinderVisitor::new(ident.name.clone());

    walk::walk_rc(&dvisitor, n.clone());

    let state = dvisitor.state.borrow();

    state.node.is_some()
}

fn find_scope<'a>(
    path: Vec<Rc<walk::Node<'a>>>,
    node: Rc<walk::Node<'a>>,
) -> Option<Rc<walk::Node<'a>>> {
    if let walk::Node::Identifier(ident) = node.as_ref() {
        let path_iter = path.iter().rev();
        for n in path_iter {
            match n.as_ref() {
                walk::Node::FunctionExpr(_)
                | walk::Node::Package(_)
                | walk::Node::File(_) => {
                    if let walk::Node::FunctionExpr(f) = n.as_ref() {
                        if function_defines(ident, f) {
                            return Some(n.clone());
                        }
                    }

                    if is_scope(ident, n.clone()) {
                        return Some(n.clone());
                    }
                }
                _ => (),
            }
        }

        if path.len() > 1 {
            return Some(path[0].clone());
        }
    }
    None
}

pub fn find_references(
    uri: String,
    position: Position,
) -> Result<Vec<Location>, String> {
    let mut locations: Vec<Location> = vec![];
    let file = utils::create_file_node(uri.clone())?;

    let result = find_node(&file, position);

    if let Some(node) = result.node {
        if let walk::Node::Identifier(ident) = node.as_ref() {
            let scope: Option<Rc<walk::Node>> =
                find_scope(result.path.clone(), node.clone());

            if let Some(scope) = scope {
                let visitor =
                    IdentFinderVisitor::new(ident.name.clone());
                walk::walk_rc(&visitor, scope);

                let state = visitor.state.borrow();
                let identifiers = (*state).identifiers.clone();

                for node in identifiers {
                    let loc = utils::map_node_to_location(
                        uri.clone(),
                        node.clone(),
                    );
                    locations.push(loc);
                }
            }
        }
    }

    Ok(locations)
}

#[derive(Default)]
pub struct FindReferencesHandler {}

impl RequestHandler for FindReferencesHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let mut locations: Vec<Location> = vec![];
        let request: Request<ReferenceParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            locations = find_references(
                params.text_document.uri,
                params.position,
            )?;
        }

        let response = Response::new(request.id, Some(locations));

        if let Ok(json) = response.to_json() {
            Ok(json)
        } else {
            Err("Could not create response json".to_string())
        }
    }
}
