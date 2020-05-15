use crate::handlers::RequestHandler;
use crate::protocol::requests::{
    HoverParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::{HoverResult, Response};

#[derive(Default)]
pub struct HoverHandler {}

#[async_trait::async_trait]
impl RequestHandler for HoverHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        let req: Request<HoverParams> =
            Request::from_json(prequest.data.as_str())?;

        let response: Response<HoverResult> =
            Response::new(req.id, None);

        Ok(Some(response.to_json()?))
    }
}
