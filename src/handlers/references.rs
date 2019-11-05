use crate::handlers::RequestHandler;
use crate::loggers::Logger;
use crate::structs::{
    Location, PolymorphicRequest, ReferenceParams, Request, Response,
};
use crate::utils;
use crate::visitors::{
    DefinitionFinderVisitor, IdentFinderVisitor, NodeFinderVisitor,
};

use std::cell::RefCell;
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

    return false;
}

fn is_scope<'a>(
    ident: &flux::ast::Identifier,
    n: Rc<walk::Node<'a>>,
) -> bool {
    let dvisitor: DefinitionFinderVisitor =
        DefinitionFinderVisitor::new(ident.name.clone());

    walk::walk_rc(&dvisitor, n.clone());

    let state = dvisitor.state.borrow();

    if let Some(_) = state.node.clone() {
        return true;
    }

    return false;
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
    return None;
}

pub struct FindReferencesHandler {
    logger: Rc<RefCell<dyn Logger>>,
}

impl RequestHandler for FindReferencesHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let mut locations: Vec<Location> = vec![];
        let request: Request<ReferenceParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let file = utils::create_file_node(uri.clone())?;
            let walker = walk::Node::File(&file);
            let visitor = NodeFinderVisitor::new(params.position);

            walk::walk(&visitor, walker);

            let state = visitor.state.borrow();
            let node = (*state).node.clone();
            let path = (*state).path.clone();

            if let Some(node) = node {
                if let walk::Node::Identifier(ident) = node.as_ref() {
                    let mut scope: Option<Rc<walk::Node>> =
                        find_scope(path.clone(), node.clone());

                    if scope.is_none() && path.len() > 1 {
                        scope = Some(path[0].clone());
                    }

                    if let Some(scope) = scope {
                        let mut logger = self.logger.borrow_mut();
                        logger.info(format!(
                            "Scope Found: {} for {}",
                            scope, node
                        ))?;

                        let visitor = IdentFinderVisitor::new(
                            ident.name.clone(),
                        );
                        walk::walk_rc(&visitor, scope);

                        let state = visitor.state.borrow();
                        let identifiers =
                            (*state).identifiers.clone();

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
        }

        let response = Response::new(request.id, Some(locations));

        if let Ok(json) = response.to_json() {
            return Ok(json);
        } else {
            return Err("Could not create response json".to_string());
        }
    }
}

impl FindReferencesHandler {
    pub fn new(
        logger: Rc<RefCell<dyn Logger>>,
    ) -> FindReferencesHandler {
        FindReferencesHandler { logger }
    }
}
