use crate::handlers::document_change::DocumentChangeHandler;
use crate::handlers::document_open::DocumentOpenHandler;
use crate::handlers::goto_definition::GotoDefinitionHandler;
use crate::handlers::initialize::InitializeHandler;
use crate::handlers::references::FindReferencesHandler;
use crate::handlers::rename::RenameHandler;
use crate::handlers::shutdown::ShutdownHandler;
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
            Box::new(FindReferencesHandler::default()),
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

        Handler { logger, mapping }
    }

    pub fn handle(
        &mut self,
        request: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        match request.method().as_str() {
            method => {
                if let Some(m) = self.mapping.get(method) {
                    match m.handle(request) {
                        Ok(r) => Ok(Some(r)),
                        Err(e) => Err(e),
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }
}
