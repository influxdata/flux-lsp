use crate::loggers::{DefaultLogger, Logger};
use crate::structs::*;

pub struct Handler {
    logger: Box<dyn Logger>,
}

impl Handler {
    pub fn new() -> Handler {
        return Handler {
            logger: Box::new(DefaultLogger {}),
        };
    }

    pub fn set_logger(&mut self, logger: Box<dyn Logger>) {
        self.logger = logger;
    }

    fn handle_initialize(&self, request: PolymorphicRequest) -> Result<String, String> {
        InitializeRequest::from_json(request.data.as_str())?;

        let result = InitializeResult::new();
        let response = InitializeResponse::new(request.base_request.id, result);

        return response.to_json();
    }

    fn handle_document_did_open(&mut self, prequest: PolymorphicRequest) -> Result<String, String> {
        let request = TextDocumentDidOpenRequest::from_json(prequest.data.as_str())?;
        self.logger.log(format!("File Opened\n"))?;
        self.logger
            .log(format!("Path: {}\n", request.params.text_document.uri))?;
        self.logger.log(format!(
            "Language: {}\n",
            request.params.text_document.language_id
        ))?;

        // TODO: read file
        // TODO: parse file
        // TODO: cache results

        return Ok(String::from(""));
    }

    pub fn handle(&mut self, request: PolymorphicRequest) -> Result<String, String> {
        match request.method().as_str() {
            "initialize" => return self.handle_initialize(request),
            "initialized" => Ok(String::from("")),
            "textDocument/didOpen" => return self.handle_document_did_open(request),
            _ => Ok(String::from("")),
        }
    }
}
