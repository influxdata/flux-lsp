use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::shared;

#[derive(Default)]
pub struct DocumentSaveHandler {}

#[async_trait::async_trait]
impl RequestHandler for DocumentSaveHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        let request = shared::parse_save_request(prequest.data)?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let msg = shared::create_diagnoistics(uri, ctx)?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didSave request".to_string())
    }
}
