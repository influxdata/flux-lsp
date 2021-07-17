#![allow(deprecated)]
use std::collections::HashMap;
use std::fs;

use futures::executor::block_on;
use lspower::lsp;
use serde_json::from_str;
use url::Url;

use crate::protocol;
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
fn get_fixture_path(name: &'static str) -> lsp::Url {
    let mut pwd = std::env::current_dir().unwrap();
    pwd.push("tests");
    pwd.push("fixtures");
    pwd.push(name);
    pwd.set_extension("flux");

    let p = pwd.as_path().to_str().unwrap().to_string();

    lsp::Url::parse(&format!("file://{}", p)).unwrap()
}

/// Read the contents of a file.
fn get_file_contents_from_uri(uri: lsp::Url) -> String {
    let file_path = Url::to_file_path(&uri).unwrap();
    fs::read_to_string(file_path).unwrap()
}

/// Open a file on the server, so it lives in memory.
fn open_file_on_server(uri: lsp::Url, router: &mut Router) {
    let text = get_file_contents_from_uri(uri.clone());
    let did_open_request = protocol::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem {
                uri: uri,
                language_id: FLUX.to_string(),
                version: 1,
                text,
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
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
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
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
    let initialize_request = protocol::Request {
        id: 1,
        params: Some(lsp::InitializeParams {
            capabilities: lsp::ClientCapabilities {
                workspace: None,
                text_document: None,
                window: None,
                general: None,
                experimental: None,
            },
            client_info: None,
            initialization_options: None,
            locale: None,
            process_id: None,
            root_path: None,
            root_uri: None,
            trace: None,
            workspace_folders: None,
        }),
        method: "initialize".to_string(),
    };

    let initialize_request_json =
        serde_json::to_string(&initialize_request).unwrap();

    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "initialize".to_string(),
        },
        data: initialize_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap()
            .unwrap();
    let expected = protocol::Response {
        id: 1,
        result: Some(lsp::InitializeResult {
            capabilities: lsp::ServerCapabilities {
                call_hierarchy_provider: None,
                code_action_provider: None,
                code_lens_provider: None,
                color_provider: None,
                completion_provider: Some(
                    lsp::CompletionOptions::default(),
                ),
                declaration_provider: None,
                definition_provider: Some(lsp::OneOf::Left(true)),
                document_formatting_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                document_highlight_provider: None,
                document_link_provider: None,
                document_on_type_formatting_provider: None,
                document_range_formatting_provider: None,
                document_symbol_provider: Some(lsp::OneOf::Left(
                    true,
                )),
                execute_command_provider: None,
                experimental: None,
                folding_range_provider: Some(
                    lsp::FoldingRangeProviderCapability::Simple(true),
                ),
                hover_provider: None,
                implementation_provider: None,
                linked_editing_range_provider: None,
                moniker_provider: None,
                references_provider: Some(lsp::OneOf::Left(true)),
                rename_provider: Some(lsp::OneOf::Left(true)),
                selection_range_provider: None,
                semantic_tokens_provider: None,
                signature_help_provider: Some(
                    lsp::SignatureHelpOptions::default(),
                ),
                text_document_sync: Some(
                    lsp::TextDocumentSyncCapability::Kind(
                        lsp::TextDocumentSyncKind::Full,
                    ),
                ),
                type_definition_provider: None,
                workspace: None,
                workspace_symbol_provider: None,
            },
            server_info: Some(lsp::ServerInfo {
                name: "flux-lsp".to_string(),
                version: Some("1.0".to_string()),
            }),
        }),
        jsonrpc: JSONRPCVERSION.to_string(),
    };
    let expected_json = expected.to_json().unwrap();

    assert_eq!(expected_json, response, "expects correct response");
}

#[test]
fn test_initialized() {
    let mut router = Router::new(false);
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
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
    let did_open_request = protocol::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem {
                uri: uri.clone(),
                language_id: FLUX.to_string(),
                version: 1,
                text: "".to_string(),
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
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
        protocol::create_diagnostics_notification(uri, vec![])
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
    let did_open_request = protocol::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem {
                uri: uri.clone(),
                language_id: FLUX.to_string(),
                version: 1,
                text,
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
        },
        data: did_open_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let diagnostics = vec![lsp::Diagnostic {
        range: lsp::Range {
            start: lsp::Position {
                character: 0,
                line: 0,
            },
            end: lsp::Position {
                character: 6,
                line: 0,
            },
        },
        message: "invalid statement: option".to_string(),
        code: Some(lsp::NumberOrString::Number(1)),
        severity: Some(lsp::DiagnosticSeverity::Error),

        code_description: None,
        data: None,
        related_information: None,
        source: None,
        tags: None,
    }];

    let expected_json =
        protocol::create_diagnostics_notification(uri, diagnostics)
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
    let did_open_request = protocol::Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem {
                uri: uri.clone(),
                language_id: FLUX.to_string(),
                version: 1,
                text,
            },
        }),
    };

    let did_open_request_json =
        serde_json::to_string(&did_open_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/didOpen".to_string(),
        },
        data: did_open_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let diagnostics = vec![lsp::Diagnostic {
        range: lsp::Range {
            start: lsp::Position {
                character: 11,
                line: 3,
            },
            end: lsp::Position {
                character: 14,
                line: 3,
            },
        },
        message: "pipe destination must be a function call"
            .to_string(),
        code: Some(lsp::NumberOrString::Number(1)),
        severity: Some(lsp::DiagnosticSeverity::Error),

        code_description: None,
        data: None,
        related_information: None,
        source: None,
        tags: None,
    }];

    let expected_json =
        protocol::create_diagnostics_notification(uri, diagnostics)
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

    let formatting_request = protocol::Request {
        id: 1,
        method: "textDocument/formatting".to_string(),
        params: Some(lsp::DocumentFormattingParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: uri.clone(),
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            options: lsp::FormattingOptions {
                tab_size: 0,
                insert_spaces: true,
                properties:
                    HashMap::<String, lsp::FormattingProperty>::new(),
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
            },
        }),
    };
    let request_json =
        serde_json::to_string(&formatting_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/formatting".to_string(),
        },
        data: request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned =
        from_str::<protocol::Response<Vec<lsp::TextEdit>>>(
            response.unwrap().as_str(),
        )
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

    let signature_help_request = protocol::Request {
        id: 1,
        method: "textDocument/signatureHelp".to_string(),
        params: Some(lsp::SignatureHelpParams {
            context: None,
            text_document_position_params:
                lsp::TextDocumentPositionParams {
                    position: lsp::Position {
                        line: 0,
                        character: 5,
                    },
                    text_document: lsp::TextDocumentIdentifier {
                        uri,
                    },
                },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        }),
    };
    let request_json =
        serde_json::to_string(&signature_help_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/signatureHelp".to_string(),
        },
        data: request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned =
        from_str::<protocol::Response<lsp::SignatureHelp>>(
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

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::TriggerForIncompleteCompletions,
                trigger_character: Some("(".to_string()),
            }),
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 8,
                    line: 4,
                },
                text_document: lsp::TextDocumentIdentifier { uri },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
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

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::TriggerForIncompleteCompletions,
                trigger_character: Some("(".to_string()),
            }),
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 8,
                    line: 2,
                },
                text_document: lsp::TextDocumentIdentifier { uri },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
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

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::TriggerForIncompleteCompletions,
                trigger_character: Some(".".to_string()),
            }),
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 2,
                    line: 0,
                },
                text_document: lsp::TextDocumentIdentifier { uri: uri2 },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
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

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::TriggerForIncompleteCompletions,
                trigger_character: Some(".".to_string()),
            }),
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 4,
                    line: 2,
                },
                text_document: lsp::TextDocumentIdentifier { uri },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
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

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: None,
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 1,
                    line: 8,
                },
                text_document: lsp::TextDocumentIdentifier {
                    uri: uri.clone(),
                },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
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
        position: lsp::Position {
            character: 1,
            line: 8,
        },
        uri,
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

    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
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

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: None,
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 10,
                    line: 16,
                },
                text_document: lsp::TextDocumentIdentifier { uri },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
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

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: Some(lsp::CompletionContext {
                trigger_kind: lsp::CompletionTriggerKind::Invoked,
                trigger_character: Some(".".to_string()),
            }),
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 5,
                    line: 16,
                },
                text_document: lsp::TextDocumentIdentifier { uri },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
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

    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
        .unwrap();
    let returned_items = returned.result.unwrap().items;

    assert_eq!(5, returned_items.len(), "expects completion items");
}

#[test]
fn test_option_function_completion() {
    let mut router = Router::new(false);
    let uri = get_fixture_path("options_function");
    open_file_on_server(uri.clone(), &mut router);

    let completion_request = protocol::Request {
        id: 1,
        method: "textDocument/completion".to_string(),
        params: Some(lsp::CompletionParams {
            context: None,
            text_document_position: lsp::TextDocumentPositionParams {
                position: lsp::Position {
                    character: 1,
                    line: 10,
                },
                text_document: lsp::TextDocumentIdentifier { uri },
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let completion_request_json =
        serde_json::to_string(&completion_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/completion".to_string(),
        },
        data: completion_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let returned =
        from_str::<protocol::Response<lsp::CompletionList>>(
            response.unwrap().as_str(),
        )
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

    let did_change_request = protocol::Request {
        id: 1,
        method: "textDocument/didChange".to_string(),
        params: Some(lsp::DidChangeTextDocumentParams {
            text_document: lsp::VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 1,
            },
            content_changes: vec![
                lsp::TextDocumentContentChangeEvent {
                    text: text,
                    range: None,
                    range_length: None,
                },
            ],
        }),
    };

    let did_change_request_json =
        serde_json::to_string(&did_change_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/didChange".to_string(),
        },
        data: did_change_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let expected_json =
        protocol::create_diagnostics_notification(uri, vec![])
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

    let did_change_request = protocol::Request {
        id: 1,
        method: "textDocument/didChange".to_string(),
        params: Some(lsp::DidChangeTextDocumentParams {
            text_document: lsp::VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 1,
            },
            content_changes: vec![
                lsp::TextDocumentContentChangeEvent {
                    text: text,
                    range: None,
                    range_length: None,
                },
            ],
        }),
    };

    let did_change_request_json =
        serde_json::to_string(&did_change_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/didChange".to_string(),
        },
        data: did_change_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();
    let diagnostics = vec![lsp::Diagnostic {
        range: lsp::Range {
            start: lsp::Position {
                character: 11,
                line: 3,
            },
            end: lsp::Position {
                character: 14,
                line: 3,
            },
        },
        message: "pipe destination must be a function call"
            .to_string(),
        code: Some(lsp::NumberOrString::Number(1)),
        severity: Some(lsp::DiagnosticSeverity::Error),

        code_description: None,
        data: None,
        related_information: None,
        source: None,
        tags: None,
    }];

    let expected_json =
        protocol::create_diagnostics_notification(uri, diagnostics)
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
    let shutdown_request: protocol::Request<
        protocol::ShutdownParams,
    > = protocol::Request {
        id: 1,
        method: "shutdown".to_string(),
        params: None,
    };

    let shutdown_request_json =
        serde_json::to_string(&shutdown_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "shutdown".to_string(),
        },
        data: shutdown_request_json,
    };

    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let expected: protocol::Response<protocol::ShutdownResult> =
        protocol::Response {
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
    let rename_request = protocol::Request {
        id: 1,
        method: "textDocument/rename".to_string(),
        params: Some(lsp::RenameParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: uri.clone(),
                },
                position: lsp::Position {
                    line: 1,
                    character: 1,
                },
            },
            new_name: new_name.clone(),
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        }),
    };

    let rename_request_json =
        serde_json::to_string(&rename_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/rename".to_string(),
        },
        data: rename_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let mut expected_changes: HashMap<lsp::Url, Vec<lsp::TextEdit>> =
        HashMap::new();

    let edits = vec![
        lsp::TextEdit {
            new_text: new_name.clone(),
            range: lsp::Range {
                start: lsp::Position {
                    line: 1,
                    character: 0,
                },
                end: lsp::Position {
                    line: 1,
                    character: 3,
                },
            },
        },
        lsp::TextEdit {
            new_text: new_name,
            range: lsp::Range {
                start: lsp::Position {
                    line: 8,
                    character: 34,
                },
                end: lsp::Position {
                    line: 8,
                    character: 37,
                },
            },
        },
    ];

    expected_changes.insert(uri, edits);

    let workspace_edit = lsp::WorkspaceEdit {
        changes: Some(expected_changes),
        document_changes: None,
        change_annotations: None,
    };

    let expected: protocol::Response<lsp::WorkspaceEdit> =
        protocol::Response {
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

    let folding_request = protocol::Request {
        id: 1,
        method: "textDocument/foldingRange".to_string(),
        params: Some(lsp::FoldingRangeParams {
            text_document: lsp::TextDocumentIdentifier { uri },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let folding_request_json =
        serde_json::to_string(&folding_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/foldingRange".to_string(),
        },
        data: folding_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let areas = vec![
        lsp::FoldingRange {
            start_line: 5,
            start_character: Some(25),
            end_line: 8,
            end_character: Some(37),
            kind: Some(lsp::FoldingRangeKind::Region),
        },
        lsp::FoldingRange {
            start_line: 14,
            start_character: Some(25),
            end_line: 14,
            end_character: Some(95),
            kind: Some(lsp::FoldingRangeKind::Region),
        },
    ];

    let expected: protocol::Response<Vec<lsp::FoldingRange>> =
        protocol::Response::new(1, Some(areas));

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

    let find_references_request = protocol::Request {
        id: 1,
        method: "textDocument/definition".to_string(),
        params: Some(lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: uri.clone(),
            },
            position: lsp::Position {
                line: 8,
                character: 35,
            },
        }),
    };

    let find_references_request_json =
        serde_json::to_string(&find_references_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/definition".to_string(),
        },
        data: find_references_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let expected: protocol::Response<lsp::Location> =
        protocol::Response {
            id: 1,
            result: Some(lsp::Location {
                uri,
                range: lsp::Range {
                    start: lsp::Position {
                        line: 1,
                        character: 0,
                    },
                    end: lsp::Position {
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

    let find_references_request = protocol::Request {
        id: 1,
        method: "textDocument/references".to_string(),
        params: Some(lsp::ReferenceParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: uri.clone(),
                },
                position: lsp::Position {
                    line: 1,
                    character: 1,
                },
            },
            context: lsp::ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let find_references_request_json =
        serde_json::to_string(&find_references_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/references".to_string(),
        },
        data: find_references_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let expected: protocol::Response<Vec<lsp::Location>> =
        protocol::Response {
            id: 1,
            result: Some(vec![
                lsp::Location {
                    uri: uri.clone(),
                    range: lsp::Range {
                        start: lsp::Position {
                            line: 1,
                            character: 0,
                        },
                        end: lsp::Position {
                            line: 1,
                            character: 3,
                        },
                    },
                },
                lsp::Location {
                    uri,
                    range: lsp::Range {
                        start: lsp::Position {
                            line: 8,
                            character: 34,
                        },
                        end: lsp::Position {
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

    let symbols_request = protocol::Request {
        id: 1,
        method: "textDocument/documentSymbol".to_string(),
        params: Some(lsp::DocumentSymbolParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: uri.clone(),
            },
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        }),
    };

    let symbols_request_json =
        serde_json::to_string(&symbols_request).unwrap();
    let request = protocol::PolymorphicRequest {
        base_request: protocol::BaseRequest {
            id: 1,
            method: "textDocument/documentSymbol".to_string(),
        },
        data: symbols_request_json,
    };
    let response =
        block_on(router.route(request, create_request_context()))
            .unwrap();

    let areas = vec![
        lsp::SymbolInformation {
            name: "from".to_string(),
            kind: lsp::SymbolKind::Function,
            deprecated: None,
            location: lsp::Location {
                uri: uri.clone(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 20,
                    },
                },
            },
            container_name: None,
            tags: None,
        },
        lsp::SymbolInformation {
            name: "bucket".to_string(),
            kind: lsp::SymbolKind::Variable,
            deprecated: None,
            location: lsp::Location {
                uri: uri.clone(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 5,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 19,
                    },
                },
            },
            container_name: None,
            tags: None,
        },
        lsp::SymbolInformation {
            name: "test".to_string(),
            kind: lsp::SymbolKind::String,
            deprecated: None,
            location: lsp::Location {
                uri,
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 13,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 19,
                    },
                },
            },
            container_name: None,
            tags: None,
        },
    ];

    let expected: protocol::Response<Vec<lsp::SymbolInformation>> =
        protocol::Response::new(1, Some(areas));

    assert_eq!(
        expected.to_json().unwrap(),
        response.unwrap(),
        "expects to find all symbols"
    );
}
