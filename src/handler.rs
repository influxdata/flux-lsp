use std::fs;

use url::Url;

use crate::loggers::{DefaultLogger, Logger};
use crate::structs::*;

use flux::ast::*;
use flux::parser::Parser;

fn parse(contents: &str) -> File {
    let mut p = Parser::new(contents);
    return p.parse_file(String::from(""));
}

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

        let file_path = match Url::parse(request.params.text_document.uri.as_str()) {
            Ok(s) => s,
            Err(e) => return Err(format!("Failed to get file path: {}", e)),
        };

        let contents = match fs::read_to_string(file_path.path()) {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to read file: {}", e)),
        };

        let file = parse(contents.as_str());
        for statement in file.body {
            self.logger
                .log(format!("\nstatement: {}\n", statement.base().location))?;
        }

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
