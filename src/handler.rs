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

    fn handle_document_open(&mut self, prequest: PolymorphicRequest) -> Result<String, String> {
        let request = TextDocumentRequest::from_json(prequest.data.as_str())?;
        let uri = request.params.text_document.uri;
        let lang = request.params.text_document.language_id;

        self.logger
            .info(format!("File Opened, type: {}, uri: {}", lang, uri.clone()))?;

        let msg = create_file_diagnostics(uri.clone())?;
        let json = msg.to_json()?;

        self.logger.info(format!("Request: {}", json.clone()))?;

        return Ok(json);
    }

    fn handle_document_change(&mut self, prequest: PolymorphicRequest) -> Result<String, String> {
        let request = TextDocumentRequest::from_json(prequest.data.as_str())?;
        let uri = request.params.text_document.uri;

        self.logger
            .info(format!("File Changed, uri: {}", uri.clone()))?;

        let msg = create_file_diagnostics(uri.clone())?;
        let json = msg.to_json()?;

        self.logger.info(format!("Request: {}", json.clone()))?;

        return Ok(json);
    }

    fn handle_unknown(&mut self, prequest: PolymorphicRequest) -> Result<String, String> {
        let msg =
            create_log_show_message_notification(format!("Unknown method {}", prequest.method()))?;

        return msg.to_json();
    }

    pub fn handle(&mut self, request: PolymorphicRequest) -> Result<String, String> {
        match request.method().as_str() {
            "initialize" => return self.handle_initialize(request),
            "initialized" => Ok(String::from("")),
            "textDocument/didOpen" => return self.handle_document_open(request),
            "textDocument/didChange" => return self.handle_document_change(request),
            _ => return self.handle_unknown(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestLogger {}
    impl Logger for TestLogger {
        fn info(&mut self, _: String) -> Result<(), String> {
            return Ok(());
        }
        fn error(&mut self, _: String) -> Result<(), String> {
            return Ok(());
        }
    }

    fn create_handler() -> Handler {
        let mut h = Handler::new();
        h.logger = Box::new(TestLogger {});

        return h;
    }

    fn flux_fixture_uri(filename: &'static str) -> String {
        let mut pwd = std::env::current_dir().unwrap();
        pwd.push("tests");
        pwd.push("fixtures");
        pwd.push(filename);
        pwd.set_extension("flux");

        let p = pwd.as_path().to_str().unwrap().to_string();

        return format!("file://{}", p);
    }

    #[test]
    fn test_unknown() {
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "unknwn".to_string(),
            },
            data: "".to_string(),
        };
        let mut handler = create_handler();

        let response = handler.handle(request).unwrap();
        let expected =
            create_log_show_message_notification("Unknown method unknwn".to_string()).unwrap();
        let expected_json = expected.to_json().unwrap();

        assert_eq!(expected_json, response, "expects show message response");
    }

    #[test]
    fn test_initialize() {
        let initialize_request = InitializeRequest {};
        let initialize_request_json = serde_json::to_string(&initialize_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "initialize".to_string(),
            },
            data: initialize_request_json,
        };

        let mut handler = create_handler();

        let response = handler.handle(request).unwrap();
        let expected = InitializeResponse {
            id: 1,
            result: InitializeResult::new(),
            jsonrpc: "2.0".to_string(),
        };
        let expected_json = expected.to_json().unwrap();

        assert_eq!(expected_json, response, "expects show message response");
    }

    #[test]
    fn test_initialized() {
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "initialized".to_string(),
            },
            data: "".to_string(),
        };
        let mut handler = create_handler();

        let response = handler.handle(request).unwrap();
        let expected_json = String::from("");

        assert_eq!(expected_json, response, "expects empty response");
    }

    #[test]
    fn test_document_open_ok() {
        let uri = flux_fixture_uri("ok");
        let did_open_request = TextDocumentRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
            params: TextDocumentParams {
                text_document: TextDocument {
                    uri: uri.clone(),
                    language_id: "flux".to_string(),
                    version: 1,
                    text: "".to_string(),
                },
            },
        };

        let did_open_request_json = serde_json::to_string(&did_open_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "textDocument/didOpen".to_string(),
            },
            data: did_open_request_json,
        };
        let mut handler = create_handler();

        let response = handler.handle(request).unwrap();
        let expected_json = create_diagnostics_notification(uri.clone(), vec![])
            .unwrap()
            .to_json()
            .unwrap();

        assert_eq!(
            expected_json, response,
            "expects publish diagnostic notification"
        );
    }

    #[test]
    fn test_document_open_error() {
        let uri = flux_fixture_uri("error");
        let did_open_request = TextDocumentRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
            params: TextDocumentParams {
                text_document: TextDocument {
                    uri: uri.clone(),
                    language_id: "flux".to_string(),
                    version: 1,
                    text: "".to_string(),
                },
            },
        };

        let did_open_request_json = serde_json::to_string(&did_open_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "textDocument/didOpen".to_string(),
            },
            data: did_open_request_json,
        };
        let mut handler = create_handler();

        let response = handler.handle(request).unwrap();
        let diagnostics = vec![Diagnostic {
            range: Range {
                start: structs::Position {
                    character: 11,
                    line: 3,
                },
                end: structs::Position {
                    character: 14,
                    line: 3,
                },
            },
            message: "pipe destination must be a function call".to_string(),
            code: 1,
            severity: 1,
        }];

        let expected_json = create_diagnostics_notification(uri.clone(), diagnostics)
            .unwrap()
            .to_json()
            .unwrap();

        assert_eq!(
            expected_json, response,
            "expects publish diagnostic notification"
        );
    }

    #[test]
    fn test_document_change_ok() {
        let uri = flux_fixture_uri("ok");
        let did_change_request = TextDocumentRequest {
            id: 1,
            method: "textDocument/didChange".to_string(),
            params: TextDocumentParams {
                text_document: TextDocument {
                    uri: uri.clone(),
                    language_id: "flux".to_string(),
                    version: 1,
                    text: "".to_string(),
                },
            },
        };

        let did_change_request_json = serde_json::to_string(&did_change_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "textDocument/didChange".to_string(),
            },
            data: did_change_request_json,
        };
        let mut handler = create_handler();

        let response = handler.handle(request).unwrap();
        let expected_json = create_diagnostics_notification(uri.clone(), vec![])
            .unwrap()
            .to_json()
            .unwrap();

        assert_eq!(
            expected_json, response,
            "expects publish diagnostic notification"
        );
    }

    #[test]
    fn test_document_change_error() {
        let uri = flux_fixture_uri("error");
        let did_change_request = TextDocumentRequest {
            id: 1,
            method: "textDocument/didChange".to_string(),
            params: TextDocumentParams {
                text_document: TextDocument {
                    uri: uri.clone(),
                    language_id: "flux".to_string(),
                    version: 1,
                    text: "".to_string(),
                },
            },
        };

        let did_change_request_json = serde_json::to_string(&did_change_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "textDocument/didChange".to_string(),
            },
            data: did_change_request_json,
        };
        let mut handler = create_handler();

        let response = handler.handle(request).unwrap();
        let diagnostics = vec![Diagnostic {
            range: Range {
                start: structs::Position {
                    character: 11,
                    line: 3,
                },
                end: structs::Position {
                    character: 14,
                    line: 3,
                },
            },
            message: "pipe destination must be a function call".to_string(),
            code: 1,
            severity: 1,
        }];

        let expected_json = create_diagnostics_notification(uri.clone(), diagnostics)
            .unwrap()
            .to_json()
            .unwrap();

        assert_eq!(
            expected_json, response,
            "expects publish diagnostic notification"
        );
    }
}
