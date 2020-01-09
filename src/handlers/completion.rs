use std::rc::Rc;

use crate::handlers::RequestHandler;
use crate::protocol::properties::Position;
use crate::protocol::requests::{
    CompletionParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::{
    CompletionList, Response,
};
use crate::stdlib::{get_stdlib_functions, Completable};
use crate::visitors::semantic::utils;
use crate::visitors::semantic::NodeFinderVisitor;

use flux::semantic::walk::{self, Node};

fn get_matches(v: String) -> Vec<Box<dyn Completable>> {
    let mut matches = vec![];
    let functions = get_stdlib_functions();

    for fun in functions.into_iter() {
        if fun.matches(v.clone()) {
            matches.push(fun);
        }
    }

    matches
}

fn get_ident_name(
    uri: String,
    position: Position,
) -> Result<Option<String>, String> {
    let pkg = utils::create_semantic_package(uri.clone())?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = NodeFinderVisitor::new(position);

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();
    let node = (*state).node.clone();

    if let Some(node) = node {
        match node.as_ref() {
            Node::Identifier(ident) => {
                let name = ident.name.clone();
                return Ok(Some(name));
            }
            Node::IdentifierExpr(ident) => {
                let name = ident.name.clone();
                return Ok(Some(name));
            }
            _ => {}
        }
    }

    Ok(None)
}

fn find_completions(
    params: CompletionParams,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let name = get_ident_name(uri, params.position)?;
    let mut items = vec![];

    if let Some(name) = name {
        let matches = get_matches(name);

        for m in matches.iter() {
            items.push(m.completion_item());
        }
    }

    Ok(CompletionList {
        is_incomplete: false,
        items,
    })
}

#[derive(Default)]
pub struct CompletionHandler {}

impl RequestHandler for CompletionHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let req: Request<CompletionParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = req.params {
            let completions = find_completions(params)?;

            let response = Response::new(
                prequest.base_request.id,
                Some(completions),
            );

            let result = response.to_json()?;

            return Ok(Some(result));
        }

        Err("invalid completion request".to_string())
    }
}
