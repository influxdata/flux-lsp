use crate::handlers::{create_file_diagnostics, RequestHandler};
use crate::loggers::Logger;
use crate::protocol::requests::{
    PolymorphicRequest, Request, TextDocumentParams,
};

use std::cell::RefCell;
use std::rc::Rc;

pub struct DocumentChangeHandler {
    logger: Rc<RefCell<dyn Logger>>,
}

impl RequestHandler for DocumentChangeHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let mut logger = self.logger.borrow_mut();
        let request: Request<TextDocumentParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = request.params {
            let uri = params.text_document.uri;

            logger.info(format!(
                "File Changed, uri: {}",
                uri.clone()
            ))?;

            let msg = create_file_diagnostics(uri.clone())?;
            let json = msg.to_json()?;

            logger.info(format!("Request: {}", json.clone()))?;

            return Ok(json);
        }

        Err("invalid textDocument/didChange request".to_string())
    }
}

impl DocumentChangeHandler {
    pub fn new(
        logger: Rc<RefCell<dyn Logger>>,
    ) -> DocumentChangeHandler {
        DocumentChangeHandler { logger }
    }
}
