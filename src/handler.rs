use crate::loggers::{DefaultLogger, Logger};
use crate::structs;
use crate::structs::*;
use crate::utils::get_file_contents_from_uri;

use flux::ast::*;
use flux::parser::parse_string;

pub struct Handler {
    pub logger: Box<dyn Logger>,
}

// TODO: figure out if all clients are zero based or if its just vim-lsp
//       if not remove the hard coded subtraction in favor of runtime options
fn map_error_to_diagnostic(error: check::Error) -> Diagnostic {
    Diagnostic {
        severity: 1,
        code: 1,
        message: error.message,
        range: Range {
            start: structs::Position {
                line: error.location.start.line - 1,
                character: error.location.start.column - 1,
            },
            end: structs::Position {
                line: error.location.end.line - 1,
                character: error.location.end.column - 1,
            },
        },
    }
}

fn map_errors_to_diagnostics(errors: Vec<check::Error>) -> Vec<Diagnostic> {
    let mut result = vec![];

    for error in errors {
        result.push(map_error_to_diagnostic(error));
    }

    return result;
}

fn create_file_diagnostics(uri: String) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let file = parse_string(
        uri.clone().as_str(),
        &get_file_contents_from_uri(uri.clone())?,
    );
    let walker = walk::Node::File(&file);

    let errors = check::check(walker);
    let diagnostics = map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri.clone(), diagnostics) {
        Ok(msg) => return Ok(msg),
        Err(e) => return Err(format!("Failed to create diagnostic: {}", e)),
    };
}

impl Handler {
    pub fn new() -> Handler {
        return Handler {
            logger: Box::new(DefaultLogger {}),
        };
    }

    fn handle_initialize(&self, request: PolymorphicRequest) -> Result<String, String> {
        InitializeRequest::from_json(request.data.as_str())?;

        let result = InitializeResult::new();
        let response = InitializeResponse::new(request.base_request.id, result);

        return response.to_json();
    }

    fn handle_document_did_open(&mut self, prequest: PolymorphicRequest) -> Result<String, String> {
        let request = TextDocumentDidOpenRequest::from_json(prequest.data.as_str())?;
        let uri = request.params.text_document.uri;
        let lang = request.params.text_document.language_id;

        self.logger
            .info(format!("File Opened, type: {}, uri: {}", lang, uri.clone()))?;

        let msg = create_file_diagnostics(uri.clone())?;
        let json = msg.to_json()?;

        self.logger.info(format!("Request: {}", json.clone()))?;

        return Ok(json);
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
