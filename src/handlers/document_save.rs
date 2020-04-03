use crate::handlers::RequestHandler;
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentSaveParams,
};
use crate::shared::create_diagnoistics;

fn parse_save_request(
    data: String,
) -> Result<Request<TextDocumentSaveParams>, String> {
    let request: Request<TextDocumentSaveParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

#[derive(Default)]
pub struct DocumentSaveHandler {}

#[async_trait::async_trait]
impl RequestHandler for DocumentSaveHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        let request = parse_save_request(prequest.data)?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let msg = create_diagnoistics(uri, ctx)?;
            let json = msg.to_json()?;

            return Ok(Some(json));
        }

        Err("invalid textDocument/didSave request".to_string())
    }
}
