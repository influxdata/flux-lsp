use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::shared;

use async_trait::async_trait;

#[derive(Default)]
pub struct DocumentCloseHandler {}

#[async_trait]
impl RequestHandler for DocumentCloseHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        shared::handle_close(prequest.data)
    }
}
