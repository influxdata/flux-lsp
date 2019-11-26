use crate::cache;
use crate::handlers::{create_file_diagnostics, RequestHandler};
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
            let text =
                utils::get_file_contents_from_uri(uri.clone())?;

            cache::force(uri.clone(), text)?;

            let msg = create_file_diagnostics(uri.clone())?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didChange request".to_string())
    }
}
