use crate::handlers::document_change::DocumentChangeHandler;
use crate::handlers::document_open::DocumentOpenHandler;
use crate::handlers::initialize::InitializeHandler;
use crate::handlers::references::FindReferencesHandler;
use crate::handlers::RequestHandler;
use crate::loggers::Logger;
use crate::structs::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Handler {
    pub logger: Rc<RefCell<dyn Logger>>,
    mapping: HashMap<String, Box<dyn RequestHandler>>,
}

impl Handler {
    pub fn new(logger: Rc<RefCell<dyn Logger>>) -> Handler {
        let mut mapping: HashMap<String, Box<dyn RequestHandler>> =
            HashMap::new();
        mapping.insert(
            "textDocument/references".to_string(),
            Box::new(FindReferencesHandler::new(logger.clone())),
        );
        mapping.insert(
            "textDocument/didChange".to_string(),
            Box::new(DocumentChangeHandler::new(logger.clone())),
        );
        mapping.insert(
            "textDocument/didOpen".to_string(),
            Box::new(DocumentOpenHandler::new(logger.clone())),
        );
        mapping.insert(
            "initialize".to_string(),
            Box::new(InitializeHandler::new()),
        );
        return Handler { logger, mapping };
    }

    pub fn handle(
        &mut self,
        request: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        match request.method().as_str() {
            method => {
                if let Some(m) = self.mapping.get(method) {
                    match m.handle(request) {
                        Ok(r) => return Ok(Some(r)),
                        Err(e) => return Err(e),
                    }
                } else {
                    return Ok(None);
                }
            }
        }
    }
}
