use crate::handlers::RequestHandler;
use crate::protocol::requests::PolymorphicRequest;
use crate::shared;

#[derive(Default)]
pub struct DocumentChangeHandler {}

impl RequestHandler for DocumentChangeHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        shared::handle_change(prequest.data)
    }
}
