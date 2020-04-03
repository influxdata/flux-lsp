use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentParams,
};

use async_trait::async_trait;

#[derive(Default)]
pub struct DocumentCloseHandler {}

fn parse_close_request(
    data: String,
) -> Result<Request<TextDocumentParams>, String> {
    let request: Request<TextDocumentParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

fn handle_close(data: String) -> Result<Option<String>, String> {
    let request = parse_close_request(data)?;

    if let Some(params) = request.params {
        let uri = params.text_document.uri;

        cache::remove(uri)?;

        return Ok(None);
    }

    Err("invalid textDocument/didClose request".to_string())
}

#[async_trait]
impl RequestHandler for DocumentCloseHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        handle_close(prequest.data)
    }
}
