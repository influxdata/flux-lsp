use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request, Response};
use crate::visitors::semantic::{utils, FoldFinderVisitor};

use flux::semantic::walk::{self, Node};

use std::rc::Rc;

use lsp_types as lsp;

fn node_to_folding_range(node: Rc<Node>) -> lsp::FoldingRange {
    lsp::FoldingRange {
        start_line: node.loc().start.line - 1,
        start_character: Some(node.loc().start.column - 1),
        end_line: node.loc().end.line - 1,
        end_character: Some(node.loc().end.column - 1),
        kind: Some(lsp::FoldingRangeKind::Region),
    }
}

fn find_foldable_areas(
    uri: lsp::Url,
    cache: &Cache,
) -> Result<Vec<lsp::FoldingRange>, String> {
    let cv = cache.get(uri.as_str())?;
    let pkg = utils::analyze_source(cv.contents.as_str())?;
    let walker = walk::Node::Package(&pkg);
    let mut visitor = FoldFinderVisitor::default();

    walk::walk(&mut visitor, Rc::new(walker));

    let mut results = vec![];
    let state = visitor.state.borrow();
    let nodes = (*state).nodes.clone();

    for node in nodes {
        results.push(node_to_folding_range(node));
    }

    Ok(results)
}

#[derive(Default)]
pub struct FoldingHandler {}

#[async_trait::async_trait]
impl RequestHandler for FoldingHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        let request: Request<lsp::FoldingRangeParams> =
            Request::from_json(prequest.data.as_str())?;
        let mut areas: Option<Vec<lsp::FoldingRange>> = None;
        if let Some(params) = request.params {
            let foldable =
                find_foldable_areas(params.text_document.uri, cache)?;
            areas = Some(foldable);
        }

        let response = Response::new(request.id, areas);
        let json = response.to_json()?;

        Ok(Some(json))
    }
}
