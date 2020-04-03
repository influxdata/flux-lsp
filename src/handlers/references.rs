use crate::handlers::find_node;
use crate::handlers::RequestHandler;
use crate::protocol::properties::{Location, Position};
use crate::protocol::requests::{
    PolymorphicRequest, ReferenceParams, Request,
};
use crate::protocol::responses::Response;
use crate::shared::conversion::map_node_to_location;
use crate::visitors::semantic::utils::create_semantic_package;
use crate::visitors::semantic::{
    DefinitionFinderVisitor, IdentFinderVisitor,
};

use flux::semantic::nodes::FunctionExpr;
use flux::semantic::walk::{self, Node};

use std::rc::Rc;

fn function_defines(name: String, f: &FunctionExpr) -> bool {
    for param in f.params.clone() {
        if param.key.name == name {
            return true;
        }
    }

    false
}

fn is_scope(name: String, n: Rc<Node<'_>>) -> bool {
    let mut dvisitor: DefinitionFinderVisitor =
        DefinitionFinderVisitor::new(name);

    walk::walk(&mut dvisitor, n.clone());

    let state = dvisitor.state.borrow();

    state.node.is_some()
}

fn find_name(node: Rc<Node<'_>>) -> Option<String> {
    match node.as_ref() {
        Node::Identifier(ident) => Some(ident.name.clone()),
        Node::IdentifierExpr(ident) => Some(ident.name.clone()),
        _ => None,
    }
}

fn find_scope<'a>(
    path: Vec<Rc<Node<'a>>>,
    node: Rc<Node<'a>>,
) -> Option<Rc<Node<'a>>> {
    let name = find_name(node.clone());

    if let Some(name) = name {
        let path_iter = path.iter().rev();
        for n in path_iter {
            match n.as_ref() {
                Node::FunctionExpr(_)
                | Node::Package(_)
                | Node::File(_) => {
                    if let Node::FunctionExpr(f) = n.as_ref() {
                        if function_defines(name.clone(), f) {
                            return Some(n.clone());
                        }
                    }

                    if is_scope(name.clone(), n.clone()) {
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
    let pkg = create_semantic_package(uri.clone())?;

    let result = find_node(Node::Package(&pkg), position);

    if let Some(node) = result.node {
        let name = find_name(node.clone());

        if let Some(name) = name {
            let scope: Option<Rc<Node>> =
                find_scope(result.path.clone(), node.clone());

            if let Some(scope) = scope {
                let mut visitor = IdentFinderVisitor::new(name);
                walk::walk(&mut visitor, scope);

                let state = visitor.state.borrow();
                let identifiers = (*state).identifiers.clone();

                for node in identifiers {
                    let loc = map_node_to_location(
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

#[async_trait::async_trait]
impl RequestHandler for FindReferencesHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
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
        let json = response.to_json()?;

        Ok(Some(json))
    }
}
