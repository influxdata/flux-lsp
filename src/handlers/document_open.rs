use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::shared;

#[derive(Default)]
pub struct DocumentOpenHandler {}

#[async_trait::async_trait]
impl RequestHandler for DocumentOpenHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        shared::handle_open(prequest.data)
    }
}
