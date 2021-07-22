use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request};
use crate::shared::create_diagnoistics;

use lsp_types as lsp;

fn parse_save_request(
    data: String,
) -> Result<Request<lsp::DidSaveTextDocumentParams>, String> {
    Request::from_json(data.as_str())
}

#[derive(Default)]
pub struct DocumentSaveHandler {}

#[async_trait::async_trait]
impl RequestHandler for DocumentSaveHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        let request = parse_save_request(prequest.data)?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let msg = create_diagnoistics(uri, ctx, cache)?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err(Error {
            msg: "invalid textDocument/didSave request".to_string(),
        })
    }
}
