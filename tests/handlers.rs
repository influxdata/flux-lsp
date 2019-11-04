extern crate flux_lsp_lib;

use flux_lsp_lib::handler::Handler;
use flux_lsp_lib::loggers::Logger;
use flux_lsp_lib::structs::*;

use std::cell::RefCell;
use std::rc::Rc;

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
    let logger = Rc::new(RefCell::new(TestLogger {}));
    let h = Handler::new(logger);

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
    let expected = None;

    assert_eq!(expected, response, "expects show message response");
}

#[test]
fn test_initialize() {
    let initialize_request = Request {
        id: 1,
        params: InitializeRequestParams {},
        method: "initialize".to_string(),
    };
    let initialize_request_json =
        serde_json::to_string(&initialize_request).unwrap();
    let request = PolymorphicRequest {
        base_request: BaseRequest {
            id: 1,
            method: "initialize".to_string(),
        },
        data: initialize_request_json,
    };

    let mut handler = create_handler();

    let response = handler.handle(request).unwrap().unwrap();
    let expected = Response {
        id: 1,
        result: InitializeResult::new(),
        jsonrpc: "2.0".to_string(),
    };
    let expected_json = expected.to_json().unwrap();

    assert_eq!(
        expected_json, response,
        "expects show message response"
    );
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
    let expected = None;

    assert_eq!(expected, response, "expects empty response");
}

#[test]
fn test_document_open_ok() {
    let uri = flux_fixture_uri("ok");
    let did_open_request = Request {
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

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = PolymorphicRequest {
        base_request: BaseRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
        },
        data: did_open_request_json,
    };
    let mut handler = create_handler();

    let response = handler.handle(request).unwrap().unwrap();
    let expected_json =
        create_diagnostics_notification(uri.clone(), vec![])
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
    let did_open_request = Request {
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

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
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
            start: Position {
                character: 11,
                line: 3,
            },
            end: Position {
                character: 14,
                line: 3,
            },
        },
        message: "pipe destination must be a function call"
            .to_string(),
        code: 1,
        severity: 1,
    }];

    let expected_json =
        create_diagnostics_notification(uri.clone(), diagnostics)
            .unwrap()
            .to_json()
            .unwrap();

    assert_eq!(
        expected_json,
        response.unwrap(),
        "expects publish diagnostic notification"
    );
}

#[test]
fn test_document_change_ok() {
    let uri = flux_fixture_uri("ok");
    let did_change_request = Request {
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

    let did_change_request_json =
        serde_json::to_string(&did_change_request).unwrap();
    let request = PolymorphicRequest {
        base_request: BaseRequest {
            id: 1,
            method: "textDocument/didChange".to_string(),
        },
        data: did_change_request_json,
    };
    let mut handler = create_handler();

    let response = handler.handle(request).unwrap();
    let expected_json =
        create_diagnostics_notification(uri.clone(), vec![])
            .unwrap()
            .to_json()
            .unwrap();

    assert_eq!(
        expected_json,
        response.unwrap(),
        "expects publish diagnostic notification"
    );
}

#[test]
fn test_document_change_error() {
    let uri = flux_fixture_uri("error");
    let did_change_request = Request {
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

    let did_change_request_json =
        serde_json::to_string(&did_change_request).unwrap();
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
            start: Position {
                character: 11,
                line: 3,
            },
            end: Position {
                character: 14,
                line: 3,
            },
        },
        message: "pipe destination must be a function call"
            .to_string(),
        code: 1,
        severity: 1,
    }];

    let expected_json =
        create_diagnostics_notification(uri.clone(), diagnostics)
            .unwrap()
            .to_json()
            .unwrap();

    assert_eq!(
        expected_json,
        response.unwrap(),
        "expects publish diagnostic notification"
    );
}

#[test]
fn test_find_references() {
    let uri = flux_fixture_uri("ok");
    let find_references_request = Request {
        id: 1,
        method: "textDocument/didChange".to_string(),
        params: ReferenceParams {
            context: ReferenceContext {},
            text_document: TextDocument {
                uri: uri.clone(),
                language_id: "flux".to_string(),
                version: 1,
                text: "".to_string(),
            },
            position: Position {
                line: 1,
                character: 1,
            },
        },
    };

    let find_references_request_json =
        serde_json::to_string(&find_references_request).unwrap();
    let request = PolymorphicRequest {
        base_request: BaseRequest {
            id: 1,
            method: "textDocument/references".to_string(),
        },
        data: find_references_request_json,
    };
    let mut handler = create_handler();

    let response = handler.handle(request).unwrap();

    let expected: Response<Vec<Location>> = Response {
        id: 1,
        result: vec![
            Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: 1,
                        character: 0,
                    },
                    end: Position {
                        line: 1,
                        character: 3,
                    },
                },
            },
            Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: 8,
                        character: 34,
                    },
                    end: Position {
                        line: 8,
                        character: 37,
                    },
                },
            },
        ],
        jsonrpc: "2.0".to_string(),
    };

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all references"
    );
}
