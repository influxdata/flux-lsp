use crate::handlers::RequestHandler;
use crate::protocol::requests::{PolymorphicRequest, Request};
use crate::protocol::responses::{CompletionItem, Response};

#[derive(Default)]
pub struct CompletionResolveHandler {}

impl RequestHandler for CompletionResolveHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let req: Request<CompletionItem> =
            Request::from_json(prequest.data.as_str())?;

        let response =
            Response::new(prequest.base_request.id, req.params);

        let json = response.to_json()?;

        Ok(Some(json))
    }
}
