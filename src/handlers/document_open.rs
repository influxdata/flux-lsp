use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentParams,
};
use crate::shared::create_diagnoistics;
use crate::shared::structs::RequestContext;

#[derive(Default)]
pub struct DocumentOpenHandler {}

fn parse_open_request(
    data: String,
) -> Result<Request<TextDocumentParams>, String> {
    let request: Request<TextDocumentParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

fn handle_open(
    data: String,
    ctx: RequestContext,
) -> Result<Option<String>, String> {
    let request = parse_open_request(data)?;

    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = params.text_document.text;

        cache::set(uri.clone(), version, text)?;
        let msg = create_diagnoistics(uri, ctx)?;

        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didOpen request".to_string())
}

#[async_trait::async_trait]
impl RequestHandler for DocumentOpenHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        handle_open(prequest.data, ctx)
    }
}
