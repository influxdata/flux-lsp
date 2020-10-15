use crate::cache::Cache;
use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::protocol::responses::{Response, ShutdownResult};

pub struct ShutdownHandler {}

#[async_trait::async_trait]
impl RequestHandler for ShutdownHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
        _: &Cache,
    ) -> Result<Option<String>, String> {
        let id = prequest.base_request.id;
        let response: Response<ShutdownResult> =
            Response::new(id, None);

        let json = response.to_json()?;
        Ok(Some(json))
    }
}

impl Default for ShutdownHandler {
    fn default() -> Self {
        ShutdownHandler {}
    }
}
