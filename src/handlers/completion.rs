use std::rc::Rc;

use crate::handlers::RequestHandler;
use crate::protocol::properties::Position;
use crate::protocol::requests::{
    CompletionParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::{
    CompletionItem, CompletionList, Response,
};
use crate::stdlib::get_stdlib;
use crate::visitors::semantic::{
    utils, CompletableFinderVisitor, ImportFinderVisitor,
    NodeFinderVisitor,
};

use flux::semantic::walk::{self, Node};

fn get_imports(
    uri: String,
    pos: Position,
) -> Result<Vec<String>, String> {
    let pkg = utils::create_completion_package(uri.clone(), pos)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
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
            Node::MemberExpr(mexpr) => {
                if let flux::semantic::nodes::Expression::Identifier(
                    ident,
                ) = &mexpr.object
                {
                    let name = ident.name.clone();
                    return Ok(Some(format!("{}.", name)));
                }
            }
            _ => {}
        }
    }

    Ok(None)
}

fn get_stdlib_completions(
    name: String,
    imports: Vec<String>,
) -> Vec<CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib();

    for c in completes.into_iter() {
        if c.matches(name.clone(), imports.clone()) {
            matches.push(c.completion_item());
        }
    }

    matches
}

fn get_user_matches(
    uri: String,
    name: String,
    imports: Vec<String>,
    pos: Position,
) -> Result<Vec<CompletionItem>, String> {
    let pkg =
        utils::create_completion_package(uri.clone(), pos.clone())?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = CompletableFinderVisitor::new(pos.clone());

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    let result = (*state)
        .completables
        .clone()
        .into_iter()
        .filter(|x| x.matches(name.clone(), imports.clone()))
        .map(|x| x.completion_item())
        .collect();

    Ok(result)
}

fn find_completions(
    params: CompletionParams,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let pos = params.position.clone();
    let name = get_ident_name(uri.clone(), params.position)?;

    let mut items: Vec<CompletionItem> = vec![];
    let imports = get_imports(uri.clone(), pos.clone())?;

    if let Some(name) = name {
        let mut stdlib_matches =
            get_stdlib_completions(name.clone(), imports.clone());
        items.append(&mut stdlib_matches);

        let mut user_matches = get_user_matches(
            uri.clone(),
            name.clone(),
            imports.clone(),
            pos.clone(),
        )?;

        items.append(&mut user_matches);
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
