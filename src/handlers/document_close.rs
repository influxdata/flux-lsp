use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::shared;

#[derive(Default)]
pub struct DocumentCloseHandler {}

impl RequestHandler for DocumentCloseHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        shared::handle_close(prequest.data)
    }
}
