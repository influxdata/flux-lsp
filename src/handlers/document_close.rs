use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentParams,
};

#[derive(Default)]
pub struct DocumentCloseHandler {}

impl RequestHandler for DocumentCloseHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let request: Request<TextDocumentParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;

            cache::remove(uri)?;

            return Ok(None);
        }

        Err("invalid textDocument/didOpen request".to_string())
    }
}
