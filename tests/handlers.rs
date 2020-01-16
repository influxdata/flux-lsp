extern crate flux_lsp;
extern crate speculate;

use serde_json::from_str;
use speculate::speculate;

use flux_lsp::handler::Handler;
use flux_lsp::protocol::notifications::*;
use flux_lsp::protocol::properties::*;
use flux_lsp::protocol::requests::*;
use flux_lsp::protocol::responses::*;
use flux_lsp::stdlib::{get_builtins, Completable, PackageResult};

use std::collections::HashMap;
use std::fs;
use url::Url;

speculate! {
    before {
        let mut handler = create_handler();
    }

    describe "unknown request" {
        it "returns correct response" {
            let request = PolymorphicRequest {
                base_request: BaseRequest {
                    id: 1,
                    method: "unknwn".to_string(),
                },
                data: "".to_string(),
            };

            let response = handler.handle(request).unwrap();
            let expected = None;

            assert_eq!(expected, response, "expects show message response");
        }
    }

    describe "Initialize" {
        it "returns correct response" {
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

            let response = handler.handle(request).unwrap().unwrap();
            let expected = Response {
                id: 1,
                result: Some(InitializeResult::new(true)),
                jsonrpc: "2.0".to_string(),
            };
            let expected_json = expected.to_json().unwrap();

            assert_eq!(
                expected_json, response,
                "expects correct response"
            );
        }
    }

    describe "Initialized" {
        it "returns correct response" {
            let request = PolymorphicRequest {
                base_request: BaseRequest {
                    id: 1,
                    method: "initialized".to_string(),
                },
                data: "".to_string(),
            };

            let response = handler.handle(request).unwrap();
            let expected = None;

            assert_eq!(expected, response, "expects empty response");
        }
    }

    describe "Document open" {
        describe "when ok" {
            before {
                let uri = flux_fixture_uri("ok");
            }

            after {
                close_file(uri.clone(), &mut handler);
            }

            it "returns correct response" {
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
        }

        describe "when there is an error" {
            before {
                let uri = flux_fixture_uri("error");
            }

            after {
                close_file(uri.clone(), &mut handler);
            }

            it "returns an error" {
                let text = get_file_contents_from_uri(uri.clone()).unwrap();
                let did_open_request = Request {
                    id: 1,
                    method: "textDocument/didOpen".to_string(),
                    params: Some(TextDocumentParams {
                        text_document: TextDocument {
                            uri: uri.clone(),
                            language_id: "flux".to_string(),
                            version: 1,
                            text,
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
        }
    }

    describe "Completion request" {
        describe "when ok" {
            before {
                let uri = flux_fixture_uri("completion");
                open_file(uri.clone(), &mut handler);
            }

            after {
                close_file(uri.clone(), &mut handler);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: None,
                        position: Position {
                            character: 1,
                            line: 6,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.clone(),
                        }
                    }),
                };

                let completion_request_json =
                    serde_json::to_string(&completion_request).unwrap();
                let request = PolymorphicRequest {
                    base_request: BaseRequest {
                        id: 1,
                        method: "textDocument/completion".to_string(),
                    },
                    data: completion_request_json,
                };
                let response = handler.handle(request).unwrap();
                let mut items = vec![
                    PackageResult {
                        full_name: "csv".to_string(),
                        name: "csv".to_string(),
                    }.completion_item()
                ];

                let mut builtins = vec![];
                get_builtins(&mut builtins);

                for b in builtins {
                    items.push(b.completion_item());
                }

                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                assert_eq!(
                    212,
                    returned_items.len(),
                    "expects completion items"
                );

                assert_eq!(
                    returned_items.first().unwrap().label,
                    "csv",
                    "returns csv"
                );

                assert_eq!(
                    returned_items.last().unwrap().label,
                    "env (self)",
                    "returns env"
                );
            }
        }
    }

    describe "Document change" {
        describe "when ok" {
            before {
                let uri = flux_fixture_uri("ok");
                open_file(uri.clone(), &mut handler);
            }

            after {
                close_file(uri.clone(), &mut handler);
            }

            it "returns the correct response" {
                let text = get_file_contents_from_uri(uri.clone()).unwrap();

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
            }
        }

        describe "when there is an error" {
            before {
                let uri = flux_fixture_uri("error");
                open_file(uri.clone(), &mut handler);
            }

            after {
                close_file(uri.clone(), &mut handler);
            }

            it "returns the correct response" {
                let text = get_file_contents_from_uri(uri.clone()).unwrap();

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
            }
        }
    }

    describe "Shutdown" {
        it "returns the correct response" {
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
    }

    describe "Rename" {
        before {
            let uri = flux_fixture_uri("ok");
            open_file(uri.clone(), &mut handler);
        }

        after {
            close_file(uri.clone(), &mut handler);
        }

        it "returns the correct response" {
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
        }
    }

    describe "Folding" {
        before {
            let uri = flux_fixture_uri("ok");
            open_file(uri.clone(), &mut handler);
        }

        after {
            close_file(uri.clone(), &mut handler);
        }

        it "returns the correct response" {
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
                "expects to find all folding regions"
            );
        }
    }

    describe "Goto definition" {
        before {
            let uri = flux_fixture_uri("ok");
            open_file(uri.clone(), &mut handler);
        }

        after {
            close_file(uri.clone(), &mut handler);
        }

        it "returns correct response" {
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
        }
    }

    describe "Find references" {
        before {
            let uri = flux_fixture_uri("ok");
            open_file(uri.clone(), &mut handler);
        }

        after {
            close_file(uri.clone(), &mut handler);
        }

        it "returns correct response" {
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
        }
    }

    describe "Document symbols" {
        before {
            let uri = flux_fixture_uri("simple");
            open_file(uri.clone(), &mut handler);
        }

        after {
            close_file(uri.clone(), &mut handler);
        }

        it "returns the correct response" {
            let symbols_request = Request {
                id: 1,
                method: "textDocument/documentSymbol".to_string(),
                params: Some(DocumentSymbolParams {
                    text_document: TextDocumentIdentifier {
                        uri: uri.clone(),
                    },
                }),
            };

            let symbols_request_json =
                serde_json::to_string(&symbols_request).unwrap();
            let request = PolymorphicRequest {
                base_request: BaseRequest {
                    id: 1,
                    method: "textDocument/documentSymbol".to_string(),
                },
                data: symbols_request_json,
            };
            let response = handler.handle(request).unwrap();

            let areas = vec![
                SymbolInformation {
                   name: "from".to_string(),
                   kind: SymbolKind::Function,
                   deprecated: Some(false),
                   location: Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 20,
                            },
                        }
                   },
                   container_name: None,
                },
                SymbolInformation {
                   name: "bucket".to_string(),
                   kind: SymbolKind::Variable,
                   deprecated: Some(false),
                   location: Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 5,
                            },
                            end: Position {
                                line: 0,
                                character: 19,
                            },
                        }
                   },
                   container_name: None,
                },
                SymbolInformation {
                   name: "test".to_string(),
                   kind: SymbolKind::String,
                   deprecated: Some(false),
                   location: Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 13,
                            },
                            end: Position {
                                line: 0,
                                character: 19,
                            },
                        }
                   },
                   container_name: None,
                }
            ];

            let expected: Response<Vec<SymbolInformation>> =
                Response::new(1, Some(areas));

            assert_eq!(
                expected.to_json().unwrap(),
                response.unwrap(),
                "expects to find all symbols"
            );
        }
    }
}

fn flux_fixture_uri(filename: &'static str) -> String {
    let mut pwd = std::env::current_dir().unwrap();
    pwd.push("tests");
    pwd.push("fixtures");
    pwd.push(filename);
    pwd.set_extension("flux");

    let p = pwd.as_path().to_str().unwrap().to_string();

    format!("file://{}", p)
}

pub fn get_file_contents_from_uri(
    uri: String,
) -> Result<String, String> {
    let url = match Url::parse(uri.as_str()) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("Failed to get file path: {}", e))
        }
    };

    let file_path = match Url::to_file_path(&url) {
        Ok(s) => s,
        Err(_) => return Err("Faild to get file_path".to_string()),
    };

    let contents = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to read file: {}", e)),
    };

    Ok(contents)
}

fn create_handler() -> Handler {
    Handler::new(false)
}

fn open_file(uri: String, handler: &mut Handler) {
    let text = get_file_contents_from_uri(uri.clone()).unwrap();
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
    let text = get_file_contents_from_uri(uri.clone()).unwrap();
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
