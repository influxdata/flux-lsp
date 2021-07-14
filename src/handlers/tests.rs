use std::collections::HashMap;
use std::fs;

use futures::executor::block_on;
use lspower::lsp;
use serde_json::from_str;
use url::Url;

use crate::protocol::notifications;
use crate::protocol::properties;
use crate::protocol::requests;
use crate::protocol::responses;
use crate::shared::callbacks::Callbacks;
use crate::shared::{CompletionInfo, CompletionType, RequestContext};
use crate::stdlib::{get_builtins, Completable, PackageResult};
use crate::Router;

const FLUX: &'static str = "flux";
const JSONRPCVERSION: &'static str = "2.0";

/// Create a blank request context
fn create_request_context() -> RequestContext {
    RequestContext {
        callbacks: Callbacks::default(),
        support_multiple_files: true,
    }
}

/// Get a uri path to a fixture file.
fn get_fixture_path(name: &'static str) -> String {
    let mut pwd = std::env::current_dir().unwrap();
    pwd.push("tests");
    pwd.push("fixtures");
    pwd.push(name);
    pwd.set_extension("flux");

    let p = pwd.as_path().to_str().unwrap().to_string();

    format!("file://{}", p)
}

/// Read the contents of a file.
fn get_file_contents_from_uri(uri: String) -> String {
    let url = Url::parse(uri.as_str()).unwrap();
    let file_path = Url::to_file_path(&url).unwrap();
    fs::read_to_string(file_path).unwrap()
}

/// Open a file on the server, so it lives in memory.
fn open_file_on_server(uri: String, router: &mut Router) {
    let text = get_file_contents_from_uri(uri.clone());
    let did_open_request = requests::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(requests::TextDocumentParams {
            text_document: properties::TextDocument {
                uri: uri,
                language_id: FLUX.to_string(),
                version: 1,
                text,
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
        },
        data: did_open_request_json,
    };

    block_on(router.route(request, create_request_context()))
        .unwrap();
}

#[test]
fn test_invalid_method() {
    let mut router = Router::new(false);
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "unknwn".to_string(),
        },
        data: "".to_string(),
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let expected = None;

    assert_eq!(expected, response, "expects show message response");
}

#[test]
fn test_initialize() {
    let mut router = Router::new(false);
    let initialize_request = requests::Request {
        id: 1,
        params: Some(requests::InitializeParams {}),
        method: "initialize".to_string(),
    };

    let initialize_request_json =
        serde_json::to_string(&initialize_request).unwrap();

    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "initialize".to_string(),
        },
        data: initialize_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap()
            .unwrap();
    let expected = responses::Response {
        id: 1,
        result: Some(responses::InitializeResult::new(true)),
        jsonrpc: JSONRPCVERSION.to_string(),
    };
    let expected_json = expected.to_json().unwrap();

    assert_eq!(expected_json, response, "expects correct response");
}

#[test]
fn test_initialized() {
    let mut router = Router::new(false);
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "initialized".to_string(),
        },
        data: "".to_string(),
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let expected = None;

    assert_eq!(expected, response, "expects empty response");
}

#[test]
fn test_open_file() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("ok");
    let did_open_request = requests::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(requests::TextDocumentParams {
            text_document: properties::TextDocument {
                uri: uri.clone(),
                language_id: FLUX.to_string(),
                version: 1,
                text: "".to_string(),
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
        },
        data: did_open_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap()
            .unwrap();
    let expected_json =
        notifications::create_diagnostics_notification(uri, vec![])
            .to_json()
            .unwrap();

    assert_eq!(
        expected_json, response,
        "expects publish diagnostic notification"
    );
}

#[test]
fn test_incomplete_option() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("incomplete_option");
    let text = get_file_contents_from_uri(uri.clone());
    let did_open_request = requests::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(requests::TextDocumentParams {
            text_document: properties::TextDocument {
                uri: uri.to_string(),
                language_id: FLUX.to_string(),
                version: 1,
                text,
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
        },
        data: did_open_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let diagnostics = vec![properties::Diagnostic {
        range: properties::Range {
            start: properties::Position {
                character: 0,
                line: 0,
            },
            end: properties::Position {
                character: 6,
                line: 0,
            },
        },
        message: "invalid statement: option".to_string(),
        code: 1,
        severity: 1,
    }];

    let expected_json =
        notifications::create_diagnostics_notification(
            uri.to_string(),
            diagnostics,
        )
        .to_json()
        .unwrap();

    assert_eq!(
        expected_json,
        response.unwrap(),
        "expects publish diagnostic notification"
    );
}

#[test]
fn test_error_on_error() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("error");
    let text = get_file_contents_from_uri(uri.clone());
    let did_open_request = requests::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(requests::TextDocumentParams {
            text_document: properties::TextDocument {
                uri: uri.to_string(),
                language_id: FLUX.to_string(),
                version: 1,
                text,
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
        },
        data: did_open_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let diagnostics = vec![properties::Diagnostic {
        range: properties::Range {
            start: properties::Position {
                character: 11,
                line: 3,
            },
            end: properties::Position {
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
        notifications::create_diagnostics_notification(
            uri.to_string(),
            diagnostics,
        )
        .to_json()
        .unwrap();

    assert_eq!(
        expected_json,
        response.unwrap(),
        "expects publish diagnostic notification"
    );
}

#[test]
fn test_formatting() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("formatting");
    open_file_on_server(uri.clone(), &mut router);

    let formatting_request = requests::Request {
        id: 1,
        method: "textDocument/formatting".to_string(),
        params: Some(requests::DocumentFormattingParams {
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };
    let request_json =
        serde_json::to_string(&formatting_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/formatting".to_string(),
        },
        data: request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned = from_str::<
        responses::Response<Vec<properties::TextEdit>>,
    >(response.unwrap().as_str())
    .unwrap();

    let result = returned.result.unwrap();
    let edit = result.first().unwrap();
    let text = edit.new_text.clone();

    let file_text = get_file_contents_from_uri(uri);
    let formatted_text = flux::formatter::format(&file_text).unwrap();

    assert_eq!(text, formatted_text, "returns formatted text");
}

#[test]
fn test_signature_help() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("signatures");
    open_file_on_server(uri.clone(), &mut router);

    let signature_help_request = requests::Request {
        id: 1,
        method: "textDocument/signatureHelp".to_string(),
        params: Some(requests::SignatureHelpParams {
            context: None,
            position: properties::Position {
                line: 0,
                character: 5,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };
    let request_json =
        serde_json::to_string(&signature_help_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/signatureHelp".to_string(),
        },
        data: request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned =
        from_str::<responses::Response<lsp::SignatureHelp>>(
            response.unwrap().as_str(),
        )
        .unwrap();

    let signatures = returned.result.unwrap().signatures;

    assert_eq!(
        signatures.len(),
        64,
        "returns the correct signatures"
    );
}

#[test]
fn test_object_param_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("object_param_completion");
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: Some(requests::CompletionContext {
                trigger_kind: 2,
                trigger_character: Some("(".to_string()),
            }),
            position: properties::Position {
                character: 8,
                line: 4,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
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

#[test]
fn test_param_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("param_completion");
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: Some(requests::CompletionContext {
                trigger_kind: 2,
                trigger_character: Some("(".to_string()),
            }),
            position: properties::Position {
                character: 8,
                line: 2,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
    let returned_items = returned.result.unwrap().items;

    let mut labels = returned_items
        .clone()
        .into_iter()
        .map(|x| x.label)
        .collect::<Vec<String>>();

    labels.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    assert_eq!(
        labels,
        vec!["csv", "file", "mode", "url"],
        "returns correct items"
    );

    assert_eq!(
        returned_items.len(),
        4,
        "returns correct number of results"
    );
}

#[test]
fn test_param_completion_multiple_files() {
    let mut router = Router::new(false);
    let uri1 = get_fixture_path("multiple_1");
    open_file_on_server(uri1.clone(), &mut router);
    let uri2 = get_fixture_path("multiple_2");
    open_file_on_server(uri2.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: Some(requests::CompletionContext {
                trigger_kind: 2,
                trigger_character: Some(".".to_string()),
            }),
            position: properties::Position {
                character: 2,
                line: 0,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri2.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
    let returned_items = returned.result.unwrap().items;

    assert_eq!(
        returned_items.len(),
        2,
        "returns correct number of results"
    );
}

#[test]
fn test_package_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("package_completion");
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: Some(requests::CompletionContext {
                trigger_kind: 2,
                trigger_character: Some(".".to_string()),
            }),
            position: properties::Position {
                character: 4,
                line: 2,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
    let returned_items = returned.result.unwrap().items;

    assert_eq!(
        returned_items.len(),
        2,
        "returns correct number of results"
    );
}

#[test]
fn test_variable_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("completion");
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: None,
            position: properties::Position {
                character: 1,
                line: 8,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let info = CompletionInfo {
        bucket: None,
        completion_type: CompletionType::Generic,
        ident: "".to_string(),
        imports: vec![],
        package: None,
        position: properties::Position {
            character: 1,
            line: 8,
        },
        uri: uri.to_string(),
    };

    let mut items = vec![block_on(
        PackageResult {
            full_name: "csv".to_string(),
            name: "csv".to_string(),
        }
        .completion_item(create_request_context(), info.clone()),
    )];

    let mut builtins = vec![];
    get_builtins(&mut builtins);

    for b in builtins {
        let item = block_on(
            b.completion_item(create_request_context(), info.clone()),
        );
        items.push(item);
    }

    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
    let returned_items = returned.result.unwrap().items;

    assert_eq!(117, returned_items.len(), "expects completion items");

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

#[test]
fn test_options_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("options"); // This should be named options_completion
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: None,
            position: properties::Position {
                character: 10,
                line: 16,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
    let returned_items = returned.result.unwrap().items;

    // This test may fail when flux is updated, as the number of
    // completion items changes. This is not an unstable test, but
    // it may be required to look at the items returned to manually
    // see which ones were added or removed when the flux stdlib
    // changes. Swap the panic line comments to do that.
    assert_eq!(
        123,
        returned_items.len(),
        //"{:#?}", returned_items,
        "expects completion items"
    );

    assert_eq!(
        returned_items.last().unwrap().label,
        "task (self)",
        "returns user defined task"
    );
}

#[test]
fn test_option_object_members_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("options_object_members");
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: Some(requests::CompletionContext {
                trigger_kind: 0,
                trigger_character: Some(".".to_string()),
            }),
            position: properties::Position {
                character: 5,
                line: 16,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response = match block_on(
        router.route(request, create_request_context()),
    ) {
        Ok(response) => response,
        Err(e) => {
            panic!("{:?}", e);
        }
    };

    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
    let returned_items = returned.result.unwrap().items;

    assert_eq!(5, returned_items.len(), "expects completion items");
}

#[test]
fn test_option_function_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("options_function");
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = requests::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(requests::CompletionParams {
            context: None,
            position: properties::Position {
                character: 1,
                line: 10,
            },
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let returned = from_str::<
        responses::Response<responses::CompletionList>,
    >(response.unwrap().as_str())
    .unwrap();
    let returned_items = returned.result.unwrap().items;

    assert_eq!(117, returned_items.len(), "expects completion items");
}

#[test]
fn test_document_change() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("ok");
    open_file_on_server(uri.clone(), &mut router);

    let text = get_file_contents_from_uri(uri.clone());

    let did_change_request = requests::Request {
        id: 1,
        method: "textDocument/didChange".to_string(),
        params: Some(requests::TextDocumentChangeParams {
            text_document:
                properties::VersionedTextDocumentIdentifier {
                    uri: uri.to_string(),
                    version: 1,
                },
            content_changes: vec![properties::ContentChange {
                text: text,
                range: None,
                range_length: None,
            }],
        }),
    };

    let did_change_request_json =
        serde_json::to_string(&did_change_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/didChange".to_string(),
        },
        data: did_change_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let expected_json =
        notifications::create_diagnostics_notification(
            uri.to_string(),
            vec![],
        )
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
    let mut router = Router::new(false);
    let uri = get_fixture_path("error");
    open_file_on_server(uri.clone(), &mut router);

    let text = get_file_contents_from_uri(uri.clone());

    let did_change_request = requests::Request {
        id: 1,
        method: "textDocument/didChange".to_string(),
        params: Some(requests::TextDocumentChangeParams {
            text_document:
                properties::VersionedTextDocumentIdentifier {
                    uri: uri.to_string(),
                    version: 1,
                },
            content_changes: vec![properties::ContentChange {
                text: text,
                range: None,
                range_length: None,
            }],
        }),
    };

    let did_change_request_json =
        serde_json::to_string(&did_change_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/didChange".to_string(),
        },
        data: did_change_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let diagnostics = vec![properties::Diagnostic {
        range: properties::Range {
            start: properties::Position {
                character: 11,
                line: 3,
            },
            end: properties::Position {
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
        notifications::create_diagnostics_notification(
            uri.to_string(),
            diagnostics,
        )
        .to_json()
        .unwrap();

    assert_eq!(
        expected_json,
        response.unwrap(),
        "expects publish diagnostic notification"
    );
}

#[test]
fn test_shutdown() {
    let mut router = Router::new(false);
    let shutdown_request: requests::Request<
        requests::ShutdownParams,
    > = requests::Request {
        id: 1,
        method: "shutdown".to_string(),
        params: None,
    };

    let shutdown_request_json =
        serde_json::to_string(&shutdown_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "shutdown".to_string(),
        },
        data: shutdown_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let expected: responses::Response<responses::ShutdownResult> =
        responses::Response {
            id: 1,
            result: None,
            jsonrpc: JSONRPCVERSION.to_string(),
        };

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all references"
    );
}

#[test]
fn test_rename() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("ok");
    open_file_on_server(uri.clone(), &mut router);

    let new_name = "environment".to_string();
    let rename_request = requests::Request {
        id: 1,
        method: "textDocument/rename".to_string(),
        params: Some(requests::RenameParams {
            text_document: properties::TextDocument {
                uri: uri.to_string(),
                language_id: FLUX.to_string(),
                version: 1,
                text: "".to_string(),
            },
            position: properties::Position {
                line: 1,
                character: 1,
            },
            new_name: new_name.clone(),
        }),
    };

    let rename_request_json =
        serde_json::to_string(&rename_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/rename".to_string(),
        },
        data: rename_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let mut expected_changes: HashMap<
        String,
        Vec<properties::TextEdit>,
    > = HashMap::new();

    let edits = vec![
        properties::TextEdit {
            new_text: new_name.clone(),
            range: properties::Range {
                start: properties::Position {
                    line: 1,
                    character: 0,
                },
                end: properties::Position {
                    line: 1,
                    character: 3,
                },
            },
        },
        properties::TextEdit {
            new_text: new_name,
            range: properties::Range {
                start: properties::Position {
                    line: 8,
                    character: 34,
                },
                end: properties::Position {
                    line: 8,
                    character: 37,
                },
            },
        },
    ];

    expected_changes.insert(uri.to_string(), edits);

    let workspace_edit = responses::WorkspaceEditResult {
        changes: expected_changes,
    };

    let expected: responses::Response<
        responses::WorkspaceEditResult,
    > = responses::Response {
        id: 1,
        result: Some(workspace_edit),
        jsonrpc: JSONRPCVERSION.to_string(),
    };

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all references"
    );
}

#[test]
fn test_folding() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("ok");
    open_file_on_server(uri.clone(), &mut router);

    let folding_request = requests::Request {
        id: 1,
        method: "textDocument/foldingRange".to_string(),
        params: Some(requests::FoldingRangeParams {
            text_document: properties::TextDocument {
                uri: uri.to_string(),
                language_id: FLUX.to_string(),
                version: 1,
                text: "".to_string(),
            },
        }),
    };

    let folding_request_json =
        serde_json::to_string(&folding_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/foldingRange".to_string(),
        },
        data: folding_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let areas = vec![
        properties::FoldingRange {
            start_line: 5,
            start_character: 25,
            end_line: 8,
            end_character: 37,
            kind: "region".to_string(),
        },
        properties::FoldingRange {
            start_line: 14,
            start_character: 25,
            end_line: 14,
            end_character: 95,
            kind: "region".to_string(),
        },
    ];

    let expected: responses::Response<Vec<properties::FoldingRange>> =
        responses::Response::new(1, Some(areas));

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all folding regions"
    );
}

#[test]
fn test_go_to_definition() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("ok");
    open_file_on_server(uri.clone(), &mut router);

    let find_references_request = requests::Request {
        id: 1,
        method: "textDocument/definition".to_string(),
        params: Some(requests::TextDocumentPositionParams {
            text_document: properties::TextDocument {
                uri: uri.to_string(),
                language_id: FLUX.to_string(),
                version: 1,
                text: "".to_string(),
            },
            position: properties::Position {
                line: 8,
                character: 35,
            },
        }),
    };

    let find_references_request_json =
        serde_json::to_string(&find_references_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/definition".to_string(),
        },
        data: find_references_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let expected: responses::Response<properties::Location> =
        responses::Response {
            id: 1,
            result: Some(properties::Location {
                uri: uri.to_string(),
                range: properties::Range {
                    start: properties::Position {
                        line: 1,
                        character: 0,
                    },
                    end: properties::Position {
                        line: 1,
                        character: 24,
                    },
                },
            }),
            jsonrpc: JSONRPCVERSION.to_string(),
        };

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find definition"
    );
}

#[test]
fn test_find_references() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("ok");
    open_file_on_server(uri.clone(), &mut router);

    let find_references_request = requests::Request {
        id: 1,
        method: "textDocument/references".to_string(),
        params: Some(requests::ReferenceParams {
            context: requests::ReferenceContext {},
            text_document: properties::TextDocument {
                uri: uri.to_string(),
                language_id: FLUX.to_string(),
                version: 1,
                text: "".to_string(),
            },
            position: properties::Position {
                line: 1,
                character: 1,
            },
        }),
    };

    let find_references_request_json =
        serde_json::to_string(&find_references_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/references".to_string(),
        },
        data: find_references_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let expected: responses::Response<Vec<properties::Location>> =
        responses::Response {
            id: 1,
            result: Some(vec![
                properties::Location {
                    uri: uri.to_string(),
                    range: properties::Range {
                        start: properties::Position {
                            line: 1,
                            character: 0,
                        },
                        end: properties::Position {
                            line: 1,
                            character: 3,
                        },
                    },
                },
                properties::Location {
                    uri: uri.to_string(),
                    range: properties::Range {
                        start: properties::Position {
                            line: 8,
                            character: 34,
                        },
                        end: properties::Position {
                            line: 8,
                            character: 37,
                        },
                    },
                },
            ]),
            jsonrpc: JSONRPCVERSION.to_string(),
        };

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all references"
    );
}

#[test]
fn test_document_symbols() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("simple");
    open_file_on_server(uri.clone(), &mut router);

    let symbols_request = requests::Request {
        id: 1,
        method: "textDocument/documentSymbol".to_string(),
        params: Some(requests::DocumentSymbolParams {
            text_document: properties::TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        }),
    };

    let symbols_request_json =
        serde_json::to_string(&symbols_request).unwrap();
    let request = requests::PolymorphicRequest {
        base_request: requests::BaseRequest {
            id: 1,
            method: "textDocument/documentSymbol".to_string(),
        },
        data: symbols_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let areas = vec![
        properties::SymbolInformation {
            name: "from".to_string(),
            kind: properties::SymbolKind::Function,
            deprecated: Some(false),
            location: properties::Location {
                uri: uri.to_string(),
                range: properties::Range {
                    start: properties::Position {
                        line: 0,
                        character: 0,
                    },
                    end: properties::Position {
                        line: 0,
                        character: 20,
                    },
                },
            },
            container_name: None,
        },
        properties::SymbolInformation {
            name: "bucket".to_string(),
            kind: properties::SymbolKind::Variable,
            deprecated: Some(false),
            location: properties::Location {
                uri: uri.to_string(),
                range: properties::Range {
                    start: properties::Position {
                        line: 0,
                        character: 5,
                    },
                    end: properties::Position {
                        line: 0,
                        character: 19,
                    },
                },
            },
            container_name: None,
        },
        properties::SymbolInformation {
            name: "test".to_string(),
            kind: properties::SymbolKind::String,
            deprecated: Some(false),
            location: properties::Location {
                uri: uri.to_string(),
                range: properties::Range {
                    start: properties::Position {
                        line: 0,
                        character: 13,
                    },
                    end: properties::Position {
                        line: 0,
                        character: 19,
                    },
                },
            },
            container_name: None,
        },
    ];

    let expected: responses::Response<
        Vec<properties::SymbolInformation>,
    > = responses::Response::new(1, Some(areas));

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all symbols"
    );
}
