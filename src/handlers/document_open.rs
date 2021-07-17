use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::requests::{PolymorphicRequest, Request};
use crate::shared::create_diagnoistics;
use crate::shared::structs::RequestContext;

use lspower::lsp;

#[derive(Default)]
pub struct DocumentOpenHandler {}

fn parse_open_request(
    data: String,
) -> Result<Request<lsp::DidOpenTextDocumentParams>, String> {
    let request: Request<lsp::DidOpenTextDocumentParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

fn handle_open(
    data: String,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Option<String>, Error> {
    let request = parse_open_request(data)?;

    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = params.text_document.text;

        cache.force(uri.as_str(), version, text)?;
        let msg = create_diagnoistics(uri, ctx, cache)?;

        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err(Error {
        msg: "invalid textDocument/didOpen request".to_string(),
    })
}

#[async_trait::async_trait]
impl RequestHandler for DocumentOpenHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        handle_open(prequest.data, ctx, cache)
    }
}
