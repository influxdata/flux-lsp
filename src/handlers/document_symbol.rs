use crate::handlers::RequestHandler;
use crate::protocol::properties::SymbolInformation;
use crate::protocol::requests::{
    DocumentSymbolParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::Response;
use crate::shared::structs::RequestContext;
use crate::visitors::semantic::utils;
use crate::visitors::semantic::SymbolsVisitor;

use flux::semantic::walk::{self, Node};
use std::rc::Rc;

fn sort_symbols(
    a: &SymbolInformation,
    b: &SymbolInformation,
) -> std::cmp::Ordering {
    let a_start = a.location.range.start.clone();
    let b_start = b.location.range.start.clone();

    if a_start.line == b_start.line {
        a_start.character.cmp(&b_start.character)
    } else {
        a_start.line.cmp(&b_start.line)
    }
}

fn find_symbols(
    uri: String,
    ctx: RequestContext,
) -> Result<Vec<SymbolInformation>, String> {
    let smp = utils::create_clean_package(uri.clone(), ctx)?;
    let pkg = Node::Package(&smp);

    let mut visitor = SymbolsVisitor::new(uri);
    walk::walk(&mut visitor, Rc::new(pkg));

    let state = visitor.state.borrow();
    let mut symbols = (*state).symbols.clone();

    symbols.sort_by(|a, b| sort_symbols(a, b));

    Ok(symbols)
}

#[derive(Default)]
pub struct DocumentSymbolHandler {}

#[async_trait::async_trait]
impl RequestHandler for DocumentSymbolHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        let request: Request<DocumentSymbolParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let symbols =
                find_symbols(params.text_document.uri, ctx)?;
            let response: Response<Vec<SymbolInformation>> =
                Response::new(request.id, Some(symbols));
            let json = response.to_json()?;

            return Ok(Some(json));
        }

        Err("missing params for textDocument/documentSymbol request"
            .to_string())
    }
}
