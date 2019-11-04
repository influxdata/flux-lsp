use crate::handlers::RequestHandler;
use crate::structs::{
    InitializeRequestParams, InitializeResult, PolymorphicRequest,
    Request, Response,
};

pub struct InitializeHandler {}

impl RequestHandler for InitializeHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let _: Request<InitializeRequestParams> =
            Request::from_json(prequest.data.as_str())?;

        let result = InitializeResult::new();
        let response =
            Response::new(prequest.base_request.id, Some(result));

        return response.to_json();
    }
}

impl InitializeHandler {
    pub fn new() -> InitializeHandler {
        InitializeHandler {}
    }
}
