use crate::cache;
use crate::format::format_str;

use crate::handlers::RequestHandler;
use crate::protocol::requests::{
    DocumentFormattingParams, FormattingOptions,
};
use crate::protocol::requests::{PolymorphicRequest, Request};
use crate::protocol::responses::Response;
use async_trait::async_trait;

#[derive(Default)]
pub struct DocumentFormattingHandler {}

#[async_trait]
impl RequestHandler for DocumentFormattingHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _ctx: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        let request: Request<DocumentFormattingParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let fo: FormattingOptions = params.options;
            let cv = cache::get(uri.clone())?;

            let msg = format_str(cv.contents.as_ref(), &fo);
            let response =
                Response::new(prequest.base_request.id, Some(msg));

            let json = match response.to_json() {
                Ok(s) => Ok(s),
                Err(_) => Err(String::from(
                    "Failed to serialize formatting response",
                )),
            };

            return Ok(Some(json?));
        }

        Err("invalid textDocument/formatting request".to_string())
    }
}
