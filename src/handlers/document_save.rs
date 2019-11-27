use crate::cache;
use crate::handlers::{create_diagnostics, RequestHandler};
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentSaveParams,
};
use crate::utils;

#[derive(Default)]
pub struct DocumentSaveHandler {}

impl RequestHandler for DocumentSaveHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let request: Request<TextDocumentSaveParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let cv = cache::get(uri.clone())?;
            let node = utils::create_file_node_from_text(
                uri.clone(),
                cv.contents,
            );

            let msg = create_diagnostics(uri.clone(), node)?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didChange request".to_string())
    }
}
