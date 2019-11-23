use crate::handlers::RequestHandler;
use crate::protocol::properties::FoldingRange;
use crate::protocol::requests::{
    FoldingRangeParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::Response;
use crate::utils::get_file_contents_from_uri;
use crate::visitors::semantic::FoldFinderVisitor;
use flux::semantic::walk::{self, Node};

use std::rc::Rc;

use flux::semantic::analyze_source;

fn node_to_folding_range(node: Rc<Node>) -> FoldingRange {
    FoldingRange {
        start_line: node.loc().start.line - 1,
        start_character: node.loc().start.column - 1,
        end_line: node.loc().end.line - 1,
        end_character: node.loc().end.column - 1,
        kind: "region".to_string(),
    }
}

fn find_foldable_areas(
    uri: String,
) -> Result<Vec<FoldingRange>, String> {
    let contents = get_file_contents_from_uri(uri)?;
    let pkg = analyze_source(contents.as_str())?;
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

impl RequestHandler for FoldingHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let request: Request<FoldingRangeParams> =
            Request::from_json(prequest.data.as_str())?;
        let mut areas: Option<Vec<FoldingRange>> = None;
        if let Some(params) = request.params {
            let foldable =
                find_foldable_areas(params.text_document.uri)?;
            areas = Some(foldable);
        }

        let response = Response::new(request.id, areas);
        let json = response.to_json()?;

        Ok(Some(json))
    }
}
