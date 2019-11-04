use crate::handlers::RequestHandler;

use crate::structs::{PolymorphicRequest, Response, ShutdownResult};

pub struct ShutdownHandler {}

impl RequestHandler for ShutdownHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let response: Response<ShutdownResult> = Response {
            id: prequest.base_request.id,
            result: None,
            jsonrpc: "2.0".to_string(),
        };

        response.to_json()
    }
}

impl ShutdownHandler {
    pub fn new() -> ShutdownHandler {
        ShutdownHandler {}
    }
}
