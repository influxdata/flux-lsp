use crate::cache;
use crate::handlers::{create_diagnostics, RequestHandler};
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentParams,
};
use crate::utils::create_file_node_from_text;

#[derive(Default)]
pub struct DocumentOpenHandler {}

impl RequestHandler for DocumentOpenHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let request: Request<TextDocumentParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let version = params.text_document.version;
            let text = params.text_document.text;

            cache::set(uri.clone(), version, text)?;
            let cv = cache::get(uri.clone())?;
            let node =
                create_file_node_from_text(uri.clone(), cv.contents);

            let msg = create_diagnostics(uri.clone(), node)?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didOpen request".to_string())
    }
}
