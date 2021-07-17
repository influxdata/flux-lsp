use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::requests::{PolymorphicRequest, Request};

use async_trait::async_trait;

use lspower::lsp;

#[derive(Default)]
pub struct DocumentCloseHandler {}

fn parse_close_request(
    data: String,
) -> Result<Request<lsp::DidCloseTextDocumentParams>, String> {
    let request: Request<lsp::DidCloseTextDocumentParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

fn handle_close(
    data: String,
    cache: &Cache,
) -> Result<Option<String>, Error> {
    let request = parse_close_request(data)?;

    if let Some(params) = request.params {
        let uri = params.text_document.uri;

        cache.remove(uri.as_str())?;

        return Ok(None);
    }

    Err(Error {
        msg: "invalid textDocument/didClose request".to_string(),
    })
}

#[async_trait]
impl RequestHandler for DocumentCloseHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        handle_close(prequest.data, cache)
    }
}
