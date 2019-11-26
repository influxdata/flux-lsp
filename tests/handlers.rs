extern crate flux_lsp_lib;

use flux_lsp_lib::handler::Handler;
use flux_lsp_lib::loggers::Logger;
use flux_lsp_lib::protocol::notifications::*;
use flux_lsp_lib::protocol::properties::*;
use flux_lsp_lib::protocol::requests::*;
use flux_lsp_lib::protocol::responses::*;
use flux_lsp_lib::utils;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn flux_fixture_uri(filename: &'static str) -> String {
    let mut pwd = std::env::current_dir().unwrap();
    pwd.push("tests");
    pwd.push("fixtures");
    pwd.push(filename);
    pwd.set_extension("flux");

    let p = pwd.as_path().to_str().unwrap().to_string();

    format!("file://{}", p)
}

struct TestLogger {}
impl Logger for TestLogger {
    fn info(&mut self, _: String) -> Result<(), String> {
        Ok(())
    }
    fn error(&mut self, _: String) -> Result<(), String> {
        Ok(())
    }
}

fn create_handler() -> Handler {
    let logger = Rc::new(RefCell::new(TestLogger {}));
    Handler::new(logger, false)
}

fn open_file(uri: String, handler: &mut Handler) {
    let text =
        utils::get_file_contents_from_uri(uri.clone()).unwrap();
    let did_open_request = Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(TextDocumentParams {
            text_document: TextDocument {
                uri: uri.clone(),
                language_id: "flux".to_string(),
                version: 1,
                text: text.clone(),
            },
        }),
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

    handler.handle(request).unwrap();
}

fn close_file(uri: String, handler: &mut Handler) {
    let text =
        utils::get_file_contents_from_uri(uri.clone()).unwrap();
    let did_close_request = Request {
        id: 1,
        method: "textDocument/didClose".to_string(),
        params: Some(TextDocumentParams {
            text_document: TextDocument {
                uri: uri.clone(),
                language_id: "flux".to_string(),
                version: 1,
                text: text.clone(),
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_close_request).unwrap();
    let request = PolymorphicRequest {
        base_request: BaseRequest {
            id: 1,
            method: "textDocument/didClose".to_string(),
        },
        data: did_open_request_json,
    };

    handler.handle(request).unwrap();
}

fn with_file_open<T>(uri: String, handler: &mut Handler, f: T)
where
    T: Fn(&mut Handler),
{
    open_file(uri.clone(), handler);
    f(handler);
    close_file(uri.clone(), handler);
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
        params: Some(InitializeParams {}),
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
        result: Some(InitializeResult::new(true)),
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
        params: Some(TextDocumentParams {
            text_document: TextDocument {
                uri: uri.clone(),
                language_id: "flux".to_string(),
                version: 1,
                text: "".to_string(),
            },
        }),
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

    close_file(uri.clone(), &mut handler);
}

#[test]
fn test_document_open_error() {
    let uri = flux_fixture_uri("error");
    let did_open_request = Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(TextDocumentParams {
            text_document: TextDocument {
                uri: uri.clone(),
                language_id: "flux".to_string(),
                version: 1,
                text: "".to_string(),
            },
        }),
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

    close_file(uri.clone(), &mut handler);
}

#[test]
fn test_document_change_ok() {
    let uri = flux_fixture_uri("ok");
    let text =
        utils::get_file_contents_from_uri(uri.clone()).unwrap();

    let mut handler = create_handler();

    with_file_open(uri.clone(), &mut handler, move |handler| {
        let did_change_request = Request {
            id: 1,
            method: "textDocument/didChange".to_string(),
            params: Some(TextDocumentChangeParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 1,
                },
                content_changes: vec![ContentChange {
                    text: text.clone(),
                    range: None,
                    range_length: None,
                }],
            }),
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
    });
}

#[test]
fn test_document_change_error() {
    let uri = flux_fixture_uri("error");
    let text =
        utils::get_file_contents_from_uri(uri.clone()).unwrap();
    let mut handler = create_handler();

    with_file_open(uri.clone(), &mut handler, move |handler| {
        let did_change_request = Request {
            id: 1,
            method: "textDocument/didChange".to_string(),
            params: Some(TextDocumentChangeParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 1,
                },
                content_changes: vec![ContentChange {
                    text: text.clone(),
                    range: None,
                    range_length: None,
                }],
            }),
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
    });
}

#[test]
fn test_find_references() {
    let uri = flux_fixture_uri("ok");
    let mut handler = create_handler();

    with_file_open(uri.clone(), &mut handler, move |handler| {
        let find_references_request = Request {
            id: 1,
            method: "textDocument/references".to_string(),
            params: Some(ReferenceParams {
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
            }),
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
        let response = handler.handle(request).unwrap();

        let expected: Response<Vec<Location>> = Response {
            id: 1,
            result: Some(vec![
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
            ]),
            jsonrpc: "2.0".to_string(),
        };

        assert_eq!(
            expected.to_json().unwrap(),
            response.unwrap(),
            "expects to find all references"
        );
    });
}

#[test]
fn test_goto_definition() {
    let uri = flux_fixture_uri("ok");
    let mut handler = create_handler();

    with_file_open(uri.clone(), &mut handler, move |handler| {
        let find_references_request = Request {
            id: 1,
            method: "textDocument/definition".to_string(),
            params: Some(TextDocumentPositionParams {
                text_document: TextDocument {
                    uri: uri.clone(),
                    language_id: "flux".to_string(),
                    version: 1,
                    text: "".to_string(),
                },
                position: Position {
                    line: 8,
                    character: 35,
                },
            }),
        };

        let find_references_request_json =
            serde_json::to_string(&find_references_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "textDocument/definition".to_string(),
            },
            data: find_references_request_json,
        };
        let response = handler.handle(request).unwrap();

        let expected: Response<Location> = Response {
            id: 1,
            result: Some(Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: 1,
                        character: 0,
                    },
                    end: Position {
                        line: 1,
                        character: 24,
                    },
                },
            }),
            jsonrpc: "2.0".to_string(),
        };

        assert_eq!(
            expected.to_json().unwrap(),
            response.unwrap(),
            "expects to find definition"
        );
    });
}

#[test]
fn test_shutdown() {
    let shutdown_request: Request<ShutdownParams> = Request {
        id: 1,
        method: "shutdown".to_string(),
        params: None,
    };

    let shutdown_request_json =
        serde_json::to_string(&shutdown_request).unwrap();
    let request = PolymorphicRequest {
        base_request: BaseRequest {
            id: 1,
            method: "shutdown".to_string(),
        },
        data: shutdown_request_json,
    };
    let mut handler = create_handler();
    let response = handler.handle(request).unwrap();

    let expected: Response<ShutdownResult> = Response {
        id: 1,
        result: None,
        jsonrpc: "2.0".to_string(),
    };

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all references"
    );
}

#[test]
fn test_rename() {
    let uri = flux_fixture_uri("ok");
    let mut handler = create_handler();

    with_file_open(uri.clone(), &mut handler, move |handler| {
        let new_name = "environment".to_string();
        let rename_request = Request {
            id: 1,
            method: "textDocument/rename".to_string(),
            params: Some(RenameParams {
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
                new_name: new_name.clone(),
            }),
        };

        let rename_request_json =
            serde_json::to_string(&rename_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "textDocument/rename".to_string(),
            },
            data: rename_request_json,
        };
        let response = handler.handle(request).unwrap();

        let mut expected_changes: HashMap<String, Vec<TextEdit>> =
            HashMap::new();

        let edits = vec![
            TextEdit {
                new_text: new_name.clone(),
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
            TextEdit {
                new_text: new_name.clone(),
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
        ];

        expected_changes.insert(uri.clone(), edits);

        let workspace_edit = WorkspaceEditResult {
            changes: expected_changes.clone(),
        };

        let expected: Response<WorkspaceEditResult> = Response {
            id: 1,
            result: Some(workspace_edit),
            jsonrpc: "2.0".to_string(),
        };

        assert_eq!(
            expected.to_json().unwrap(),
            response.unwrap(),
            "expects to find all references"
        );
    });
}

#[test]
fn test_folding() {
    let uri = flux_fixture_uri("ok");
    let mut handler = create_handler();
    with_file_open(uri.clone(), &mut handler, move |handler| {
        let folding_request = Request {
            id: 1,
            method: "textDocument/foldingRange".to_string(),
            params: Some(FoldingRangeParams {
                text_document: TextDocument {
                    uri: uri.clone(),
                    language_id: "flux".to_string(),
                    version: 1,
                    text: "".to_string(),
                },
            }),
        };

        let folding_request_json =
            serde_json::to_string(&folding_request).unwrap();
        let request = PolymorphicRequest {
            base_request: BaseRequest {
                id: 1,
                method: "textDocument/foldingRange".to_string(),
            },
            data: folding_request_json,
        };
        let response = handler.handle(request).unwrap();

        let areas = vec![
            FoldingRange {
                start_line: 5,
                start_character: 25,
                end_line: 8,
                end_character: 37,
                kind: "region".to_string(),
            },
            FoldingRange {
                start_line: 14,
                start_character: 25,
                end_line: 14,
                end_character: 95,
                kind: "region".to_string(),
            },
        ];

        let expected: Response<Vec<FoldingRange>> =
            Response::new(1, Some(areas));

        assert_eq!(
            expected.to_json().unwrap(),
            response.unwrap(),
            "expects to find all references"
        );
    });
}
