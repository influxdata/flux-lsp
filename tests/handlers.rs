extern crate flux_lsp;
extern crate speculate;

use serde_json::from_str;
use speculate::speculate;

use flux_lsp::protocol::notifications::*;
use flux_lsp::protocol::properties::*;
use flux_lsp::protocol::requests::*;
use flux_lsp::protocol::responses::*;
use flux_lsp::shared::callbacks::Callbacks;
use flux_lsp::shared::RequestContext;
use flux_lsp::shared::{CompletionInfo, CompletionType};
use flux_lsp::stdlib::{get_builtins, Completable, PackageResult};
use flux_lsp::Router;

use futures::executor::block_on;

use std::collections::HashMap;
use std::fs;
use url::Url;

fn create_request_context() -> RequestContext {
    RequestContext {
        callbacks: Callbacks::default(),
        support_multiple_files: true,
    }
}

speculate! {
    before {
        let mut router = create_router();
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

            let response = block_on(router.route(request, create_request_context())).unwrap();
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

            let response = block_on(router.route(request, create_request_context())).unwrap().unwrap();
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

            let response = block_on(router.route(request, create_request_context())).unwrap();
            let expected = None;

            assert_eq!(expected, response, "expects empty response");
        }
    }

    describe "Document open" {
        describe "when ok" {
            before {
                let uri = flux_fixture_uri("ok");
                let uri = uri.as_str();
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns correct response" {
                let did_open_request = Request {
                    id: 1,
                    method: "textDocument/didOpen".to_string(),
                    params: Some(TextDocumentParams {
                        text_document: TextDocument {
                            uri: uri.to_string(),
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

                let response = block_on(router.route(request, create_request_context())).unwrap().unwrap();
                let expected_json =
                    create_diagnostics_notification(uri.to_string(), vec![])
                    .to_json()
                    .unwrap();

                assert_eq!(
                    expected_json, response,
                    "expects publish diagnostic notification"
                );
            }
        }

        describe "when incomplete option" {
            before {
                let uri = flux_fixture_uri("incomplete_option");
                let uri = uri.as_str();
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns an error" {
                let text = get_file_contents_from_uri(uri).unwrap();
                let did_open_request = Request {
                    id: 1,
                    method: "textDocument/didOpen".to_string(),
                    params: Some(TextDocumentParams {
                        text_document: TextDocument {
                            uri: uri.to_string(),
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

                let response = block_on(router.route(request, create_request_context())).unwrap();
                let diagnostics = vec![Diagnostic {
                    range: Range {
                        start: Position {
                            character: 0,
                            line: 0,
                        },
                        end: Position {
                            character: 6,
                            line: 0,
                        },
                    },
                    message: "invalid statement: option"
                        .to_string(),
                        code: 1,
                        severity: 1,
                }];

                let expected_json =
                    create_diagnostics_notification(uri.to_string(), diagnostics)
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
                let uri = uri.as_str();
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns an error" {
                let text = get_file_contents_from_uri(uri).unwrap();
                let did_open_request = Request {
                    id: 1,
                    method: "textDocument/didOpen".to_string(),
                    params: Some(TextDocumentParams {
                        text_document: TextDocument {
                            uri: uri.to_string(),
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

                let response = block_on(router.route(request, create_request_context())).unwrap();
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
                    create_diagnostics_notification(uri.to_string(), diagnostics)
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

    describe "Formatting request" {
        describe "when ok" {
            before {
                let uri = flux_fixture_uri("formatting");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }
            it "returns the correct result" {
                let formatting_request = Request {
                    id: 1,
                    method: "textDocument/formatting".to_string(),
                    params: Some(DocumentFormattingParams {
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
                        }
                    }),
                };
                let request_json = serde_json::to_string(&formatting_request).unwrap();
                let request = PolymorphicRequest {
                    base_request: BaseRequest {
                        id: 1,
                        method: "textDocument/formatting".to_string(),
                    },
                    data: request_json,
                };

                let response = block_on(router.route(request, create_request_context())).unwrap();
                let returned = from_str::<Response<Vec<TextEdit>>>(response.unwrap().as_str()).unwrap();

                let result = returned.result.unwrap();
                let edit = result.first().unwrap();
                let text = edit.new_text.clone();


                let file_text = get_file_contents_from_uri(uri).unwrap();
                let formatted_text = flux::formatter::format(&file_text).unwrap();

                assert_eq!(text, formatted_text, "returns formatted text");
            }
        }
    }

    describe "Signature help request" {
        describe "when ok" {
            before {
                let uri = flux_fixture_uri("signatures");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let signature_help_request = Request {
                    id: 1,
                    method: "textDocument/signatureHelp".to_string(),
                    params: Some(SignatureHelpParams {
                        context: None,
                        position: Position {
                            line: 0,
                            character: 5,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
                        }
                    }),
                };
                let request_json = serde_json::to_string(&signature_help_request).unwrap();
                let request = PolymorphicRequest {
                    base_request: BaseRequest {
                        id: 1,
                        method: "textDocument/signatureHelp".to_string(),
                    },
                    data: request_json,
                };

                let response = block_on(router.route(request, create_request_context())).unwrap();
                let returned = from_str::<Response<SignatureHelp>>(response.unwrap().as_str()).unwrap();

                let signatures = returned.result.unwrap().signatures;

                assert_eq!(
                    signatures.len(),
                    64,
                    "returns the correct signatures");
            }
        }
    }

    describe "Completion request" {
        describe "when object completing params" {
            before {

                let uri = flux_fixture_uri("object_param_completion");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: Some(CompletionContext {
                            trigger_kind: 2,
                            trigger_character: Some("(".to_string()),
                        }),
                        position: Position {
                            character: 8,
                            line: 4,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
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

                let response = block_on(router.route(request, create_request_context())).unwrap();
                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                let mut labels = returned_items
                    .clone()
                    .into_iter()
                    .map(|x| x.label)
                    .collect::<Vec<String>>();

                labels.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

                assert_eq!(labels, vec!["age", "name"], "returns correct items");

                assert_eq!(
                    returned_items.len(),
                    2,
                    "returns correct number of results"
                );
            }
        }

        describe "when completing params" {
            before {

                let uri = flux_fixture_uri("param_completion");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: Some(CompletionContext {
                            trigger_kind: 2,
                            trigger_character: Some("(".to_string()),
                        }),
                        position: Position {
                            character: 8,
                            line: 2,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
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

                let response = block_on(router.route(request, create_request_context())).unwrap();
                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                let mut labels = returned_items
                    .clone()
                    .into_iter()
                    .map(|x| x.label)
                    .collect::<Vec<String>>();

                labels.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

                assert_eq!(labels, vec!["csv", "file", "url"], "returns correct items");

                assert_eq!(
                    returned_items.len(),
                    3,
                    "returns correct number of results"
                );
            }
        }

        describe "when there are multiple files" {
            before {

                let uri1 = flux_fixture_uri("multiple_1");
                let uri1 = uri1.as_str();
                let uri2 = flux_fixture_uri("multiple_2");
                let uri2 = uri2.as_str();

                open_file(uri1, &mut router);
                open_file(uri2, &mut router);
            }

            after {
                close_file(uri1, &mut router);
                close_file(uri2, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: Some(CompletionContext {
                            trigger_kind: 2,
                            trigger_character: Some(".".to_string()),
                        }),
                        position: Position {
                            character: 2,
                            line: 0,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri2.to_string(),
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
                let response = block_on(router.route(request, create_request_context())).unwrap();
                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                assert_eq!(
                    returned_items.len(),
                    2,
                    "returns correct number of results"
                );
            }
        }
        describe "when completion a package" {
            before {
                let uri = flux_fixture_uri("package_completion");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: Some(CompletionContext {
                            trigger_kind: 2,
                            trigger_character: Some(".".to_string()),
                        }),
                        position: Position {
                            character: 4,
                            line: 2,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
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
                let response = block_on(router.route(request, create_request_context())).unwrap();
                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                assert_eq!(
                    returned_items.len(),
                    2,
                    "returns correct number of results"
                );
            }
        }

        describe "when ok" {
            before {
                let uri = flux_fixture_uri("completion");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: None,
                        position: Position {
                            character: 1,
                            line: 8,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
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
                let response = block_on(router.route(request, create_request_context())).unwrap();

                let info = CompletionInfo {
                    bucket: None,
                    completion_type: CompletionType::Generic,
                    ident: "".to_string(),
                    imports: vec![],
                    package: None,
                    position: Position {
                            character: 1,
                            line: 8,
                        },
                        uri: uri.to_string(),
                };


                let mut items = vec![
                    block_on(PackageResult {
                        full_name: "csv".to_string(),
                        name: "csv".to_string(),
                    }.completion_item(create_request_context(), info.clone()))
                ];

                let mut builtins = vec![];
                get_builtins(&mut builtins);

                for b in builtins {
                    let item = block_on(b.completion_item(create_request_context(), info.clone()));
                    items.push(item);
                }

                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                assert_eq!(
                    116,
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
                    "cool (self)",
                    "returns user defined function"
                );
            }
        }

        describe "when an option can be completed" {
            before {
                let uri = flux_fixture_uri("options");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: None,
                        position: Position {
                            character: 10,
                            line: 16,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
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
                let response = block_on(router.route(request, create_request_context())).unwrap();

                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                assert_eq!(
                    120,
                    returned_items.len(),
                    "expects completion items"
                );

                assert_eq!(
                    returned_items.last().unwrap().label,
                    "task (self)",
                    "returns user defined task"
                );
            }
        }

        describe "when an option members can be completed" {
            before {
                let uri = flux_fixture_uri("options_object_members");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: Some(CompletionContext{
                            trigger_kind: 0,
                            trigger_character: Some(".".to_string()),
                        }),
                        position: Position {
                            character: 5,
                            line: 16,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
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
                let response = match block_on(router.route(request, create_request_context())) {
                    Ok(response) => response,
                    Err(e) => {
                        panic!(e);
                    }
                };

                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                assert_eq!(
                    5,
                    returned_items.len(),
                    "expects completion items"
                );
            }
        }

        describe "when an option functions can be completed" {
            before {
                let uri = flux_fixture_uri("options_function");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let completion_request = Request {
                    id: 1,
                    method: "textDocument/completion".to_string(),
                    params: Some(CompletionParams {
                        context: None,
                        position: Position {
                            character: 1,
                            line: 10,
                        },
                        text_document: TextDocumentIdentifier {
                            uri: uri.to_string(),
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
                let response = block_on(router.route(request, create_request_context())).unwrap();

                let returned = from_str::<Response<CompletionList>>(response.unwrap().as_str()).unwrap();
                let returned_items = returned.result.unwrap().items;

                assert_eq!(
                    116,
                    returned_items.len(),
                    "expects completion items"
                );
            }
        }
    }

    describe "Document change" {
        describe "when ok" {
            before {
                let uri = flux_fixture_uri("ok");
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let text = get_file_contents_from_uri(uri).unwrap();

                let did_change_request = Request {
                    id: 1,
                    method: "textDocument/didChange".to_string(),
                    params: Some(TextDocumentChangeParams {
                        text_document: VersionedTextDocumentIdentifier {
                            uri: uri.to_string(),
                            version: 1,
                        },
                        content_changes: vec![ContentChange {
                            text: text,
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
                let response = block_on(router.route(request, create_request_context())).unwrap();
                let expected_json =
                    create_diagnostics_notification(uri.to_string(), vec![])
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
                let uri = uri.as_str();
                open_file(uri, &mut router);
            }

            after {
                close_file(uri, &mut router);
            }

            it "returns the correct response" {
                let text = get_file_contents_from_uri(uri).unwrap();

                let did_change_request = Request {
                    id: 1,
                    method: "textDocument/didChange".to_string(),
                    params: Some(TextDocumentChangeParams {
                        text_document: VersionedTextDocumentIdentifier {
                            uri: uri.to_string(),
                            version: 1,
                        },
                        content_changes: vec![ContentChange {
                            text: text,
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
                let response = block_on(router.route(request, create_request_context())).unwrap();
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
                    create_diagnostics_notification(uri.to_string(), diagnostics)
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

            let response = block_on(router.route(request, create_request_context())).unwrap();

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
            let uri = uri.as_str();
            open_file(uri, &mut router);
        }

        after {
            close_file(uri, &mut router);
        }

        it "returns the correct response" {
            let new_name = "environment".to_string();
            let rename_request = Request {
                id: 1,
                method: "textDocument/rename".to_string(),
                params: Some(RenameParams {
                    text_document: TextDocument {
                        uri: uri.to_string(),
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
            let response = block_on(router.route(request, create_request_context())).unwrap();

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
                    new_text: new_name,
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

            expected_changes.insert(uri.to_string(), edits);

            let workspace_edit = WorkspaceEditResult {
                changes: expected_changes,
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
            let uri = uri.as_str();
            open_file(uri, &mut router);
        }

        after {
            close_file(uri, &mut router);
        }

        it "returns the correct response" {
            let folding_request = Request {
                id: 1,
                method: "textDocument/foldingRange".to_string(),
                params: Some(FoldingRangeParams {
                    text_document: TextDocument {
                        uri: uri.to_string(),
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
            let response = block_on(router.route(request, create_request_context())).unwrap();

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
            let uri = uri.as_str();
            open_file(uri, &mut router);
        }

        after {
            close_file(uri, &mut router);
        }

        it "returns correct response" {
            let find_references_request = Request {
                id: 1,
                method: "textDocument/definition".to_string(),
                params: Some(TextDocumentPositionParams {
                    text_document: TextDocument {
                        uri: uri.to_string(),
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
            let response = block_on(router.route(request, create_request_context())).unwrap();

            let expected: Response<Location> = Response {
                id: 1,
                result: Some(Location {
                    uri: uri.to_string(),
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
            let uri = uri.as_str();
            open_file(uri, &mut router);
        }

        after {
            close_file(uri, &mut router);
        }

        it "returns correct response" {
            let find_references_request = Request {
                id: 1,
                method: "textDocument/references".to_string(),
                params: Some(ReferenceParams {
                    context: ReferenceContext {},
                    text_document: TextDocument {
                        uri: uri.to_string(),
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
            let response = block_on(router.route(request, create_request_context())).unwrap();

            let expected: Response<Vec<Location>> = Response {
                id: 1,
                result: Some(vec![
                    Location {
                        uri: uri.to_string(),
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
                        uri: uri.to_string(),
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
            let uri = uri.as_str();
            open_file(uri, &mut router);
        }

        after {
            close_file(uri, &mut router);
        }

        it "returns the correct response" {
            let symbols_request = Request {
                id: 1,
                method: "textDocument/documentSymbol".to_string(),
                params: Some(DocumentSymbolParams {
                    text_document: TextDocumentIdentifier {
                        uri: uri.to_string(),
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
            let response = block_on(router.route(request, create_request_context())).unwrap();

            let areas = vec![
                SymbolInformation {
                   name: "from".to_string(),
                   kind: SymbolKind::Function,
                   deprecated: Some(false),
                   location: Location {
                        uri: uri.to_string(),
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
                        uri: uri.to_string(),
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
                        uri: uri.to_string(),
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
    uri: &'_ str,
) -> Result<String, String> {
    let url = match Url::parse(uri) {
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

fn create_router() -> Router {
    Router::new(false)
}

fn open_file(uri: &'_ str, router: &mut Router) {
    let text = get_file_contents_from_uri(uri).unwrap();
    let did_open_request = Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(TextDocumentParams {
            text_document: TextDocument {
                uri: uri.to_string(),
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

    block_on(router.route(request, create_request_context()))
        .unwrap();
}

fn close_file(uri: &'_ str, router: &mut Router) {
    let text = get_file_contents_from_uri(uri).unwrap();
    let did_close_request = Request {
        id: 1,
        method: "textDocument/didClose".to_string(),
        params: Some(TextDocumentParams {
            text_document: TextDocument {
                uri: uri.to_string(),
                language_id: "flux".to_string(),
                version: 1,
                text,
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

    block_on(router.route(request, create_request_context()))
        .unwrap();
}
