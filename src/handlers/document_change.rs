use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::requests::PolymorphicRequest;
use crate::protocol::requests::{Request, TextDocumentChangeParams};
use crate::shared::create_diagnoistics;
use crate::shared::structs::RequestContext;

use async_trait::async_trait;

use lspower::lsp;

#[derive(Default)]
pub struct DocumentChangeHandler {}

fn apply_changes(
    original: String,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
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
    cache: &Cache,
) -> Result<Option<String>, Error> {
    let request = parse_change_request(data)?;
    if let Some(params) = request.params {
        let uri = params.text_document.uri;
        let changes = params.content_changes;
        let version = params.text_document.version;

        let cv = cache.get(uri.as_str())?;
        let text = apply_changes(cv.contents, changes);

        cache.set(uri.as_str(), version, text)?;

        let msg = create_diagnoistics(uri, ctx, cache)?;
        let json = msg.to_json()?;

        return Ok(Some(json));
    }

    Err(Error {
        msg: "invalid textDocument/didChange request".to_string(),
    })
}

#[async_trait]
impl RequestHandler for DocumentChangeHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        handle_change(prequest.data, ctx, cache)
    }
}
