use crate::handlers::document_change::DocumentChangeHandler;
use crate::handlers::document_open::DocumentOpenHandler;
use crate::handlers::goto_definition::GotoDefinitionHandler;
use crate::handlers::initialize::InitializeHandler;
use crate::handlers::references::FindReferencesHandler;
use crate::handlers::rename::RenameHandler;
use crate::handlers::shutdown::ShutdownHandler;
use crate::handlers::RequestHandler;
use crate::loggers::Logger;
use crate::protocol::requests::PolymorphicRequest;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Handler {
    pub logger: Rc<RefCell<dyn Logger>>,
    mapping: HashMap<String, Box<dyn RequestHandler>>,
}

#[derive(Default)]
struct NoOpHandler {}

impl RequestHandler for NoOpHandler {
    fn handle(
        &self,
        _: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        Ok(None)
    }
}

impl Handler {
    pub fn new(logger: Rc<RefCell<dyn Logger>>) -> Handler {
        let mut mapping: HashMap<String, Box<dyn RequestHandler>> =
            HashMap::new();
        mapping.insert(
            "textDocument/references".to_string(),
            Box::new(FindReferencesHandler::default()),
        );
        mapping.insert(
            "textDocument/didChange".to_string(),
            Box::new(DocumentChangeHandler::default()),
        );
        mapping.insert(
            "textDocument/didSave".to_string(),
            Box::new(DocumentChangeHandler::default()),
        );
        mapping.insert(
            "textDocument/didOpen".to_string(),
            Box::new(DocumentOpenHandler::default()),
        );
        mapping.insert(
            "textDocument/definition".to_string(),
            Box::new(GotoDefinitionHandler::default()),
        );
        mapping.insert(
            "textDocument/rename".to_string(),
            Box::new(RenameHandler::default()),
        );
        mapping.insert(
            "initialize".to_string(),
            Box::new(InitializeHandler::default()),
        );
        mapping.insert(
            "shutdown".to_string(),
            Box::new(ShutdownHandler::default()),
        );
        mapping.insert(
            "textDocument/foldingRange".to_string(),
            Box::new(NoOpHandler::default()),
        );

        Handler { logger, mapping }
    }

    pub fn handle(
        &mut self,
        request: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let req = request.clone();

        let mut logger = self.logger.borrow_mut();
        logger.info(format!("Request -> {:?}", req.data))?;

        let method = request.method();
        if let Some(m) = self.mapping.get(&method) {
            let resp = m.handle(request)?;

            if let Some(resp) = resp.clone() {
                logger.info(format!("Response -> {}", resp))?;
            }

            Ok(resp)
        } else {
            Ok(None)
        }
    }
}
