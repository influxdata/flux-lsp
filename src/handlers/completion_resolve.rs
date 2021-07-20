use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request, Response};

use async_trait::async_trait;

use lspower::lsp;

#[derive(Default)]
pub struct CompletionResolveHandler {}

#[async_trait]
impl RequestHandler for CompletionResolveHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
        _: &Cache,
    ) -> Result<Option<String>, Error> {
        let req: Request<lsp::CompletionItem> =
            Request::from_json(prequest.data.as_str())?;

        let response =
            Response::new(prequest.base_request.id, req.params);

        let json = response.to_json()?;

        Ok(Some(json))
    }
}
