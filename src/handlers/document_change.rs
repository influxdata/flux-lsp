use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::shared;

#[derive(Default)]
pub struct DocumentChangeHandler {}

impl RequestHandler for DocumentChangeHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let request = shared::parse_change_request(prequest.data)?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let changes = params.content_changes;
            let version = params.text_document.version;

            let cv = cache::get(uri.clone())?;
            let text = shared::apply_changes(cv.contents, changes);

            cache::set(uri.clone(), version, text.clone())?;

            let msg = shared::create_diagnoistics(
                uri.clone(),
                text.clone(),
            )?;

            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didChange request".to_string())
    }
}
