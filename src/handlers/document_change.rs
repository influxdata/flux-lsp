use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::properties::ContentChange;
use crate::protocol::requests::PolymorphicRequest;
use crate::protocol::requests::{Request, TextDocumentChangeParams};
use crate::shared::create_diagnoistics;
use crate::shared::structs::RequestContext;

use async_trait::async_trait;

#[derive(Default)]
pub struct DocumentChangeHandler {}

fn apply_changes(
    original: String,
    changes: Vec<ContentChange>,
) -> String {
    for change in changes {
        if change.range.is_none() {
            return change.text;
        }
    }

    original
}

fn parse_change_request(
    data: String,
) -> Result<Request<TextDocumentChangeParams>, String> {
    let request: Request<TextDocumentChangeParams> =
        Request::from_json(data.as_str())?;

    Ok(request)
}

fn handle_change(
    data: String,
    ctx: RequestContext,
) -> Result<Option<String>, String> {
    let request = parse_change_request(data)?;
    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let changes = params.content_changes;
        let version = params.text_document.version;

        let cv = cache::get(uri.clone())?;
        let text = apply_changes(cv.contents, changes);

        cache::set(uri.clone(), version, text)?;

        let msg = create_diagnoistics(uri, ctx)?;
        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err("invalid textDocument/didChange request".to_string())
}

#[async_trait]
impl RequestHandler for DocumentChangeHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        handle_change(prequest.data, ctx)
    }
}
