use crate::cache;
use crate::handlers::{create_diagnostics, RequestHandler};
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentChangeParams,
};
use crate::utils::create_file_node_from_text;

#[derive(Default)]
pub struct DocumentChangeHandler {}

impl RequestHandler for DocumentChangeHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let request: Request<TextDocumentChangeParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let changes = params.content_changes.clone();
            let uri = params.text_document.uri;
            let version = params.text_document.version;

            cache::apply(uri.clone(), version, changes.clone())?;
            let cv = cache::get(uri.clone())?;

            let file =
                create_file_node_from_text(uri.clone(), cv.contents);
            let msg = create_diagnostics(uri.clone(), file)?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didChange request".to_string())
    }
}
