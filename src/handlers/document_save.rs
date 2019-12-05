use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::shared;

#[derive(Default)]
pub struct DocumentSaveHandler {}

impl RequestHandler for DocumentSaveHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let request = shared::parse_save_request(prequest.data)?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let cv = cache::get(uri.clone())?;
            let msg = shared::create_diagnoistics(uri, cv.contents)?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didSave request".to_string())
    }
}
