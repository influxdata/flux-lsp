use crate::handlers::RequestHandler;
use crate::protocol::requests::{
    InitializeParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::{InitializeResult, Response};

pub struct InitializeHandler {}

impl RequestHandler for InitializeHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let _: Request<InitializeParams> =
            Request::from_json(prequest.data.as_str())?;

        let result = InitializeResult::default();
        let response =
            Response::new(prequest.base_request.id, Some(result));

        response.to_json()
    }
}

impl Default for InitializeHandler {
    fn default() -> Self {
        InitializeHandler {}
    }
}
