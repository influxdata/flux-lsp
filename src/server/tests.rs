#![allow(deprecated, clippy::panic, clippy::unwrap_used)]
use std::collections::{BTreeSet, HashMap};

use async_std::test;
use expect_test::expect;
use lspower::{lsp, LanguageServer};

use super::*;

/// Finds a `// ^` comment in `source` and returns the `lsp::Position` that the comment points
/// at
fn position_of(source: &str) -> lsp::Position {
    source
        .lines()
        .enumerate()
        .find_map(|(line, line_str)| {
            line_str.find("// ^").map(|j| lsp::Position {
                // The marker is on the line after the position we indicate
                line: line as u32 - 1,
                character: (line_str[..j].chars().count()
                    + "// ^".len()) as u32,
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "Could not find the position marker `// ^` in `{}`",
                source
            )
        })
}

fn create_server() -> LspServer {
    let _ = env_logger::try_init();
    LspServer::new(None)
}

async fn open_file(
    server: &LspServer,
    text: String,
    filename: Option<&str>,
) {
    let name = match filename {
        Some(name) => name,
        None => "file:///home/user/file.flux",
    };

    let params = lsp::DidOpenTextDocumentParams {
        text_document: lsp::TextDocumentItem::new(
            lsp::Url::parse(name).unwrap(),
            "flux".to_string(),
            1,
            text,
        ),
    };
    server.did_open(params).await;
}

#[test]
async fn test_initialized() {
    let server = create_server();

    let params = lsp::InitializeParams {
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
    };

    let result = server.initialize(params).await.unwrap();
    let server_info = result.server_info.unwrap();

    assert_eq!(server_info.name, "flux-lsp".to_string());
    assert_eq!(
        server_info.version,
        Some(env!("CARGO_PKG_VERSION").into())
    );
}

#[test]
async fn test_shutdown() {
    let server = create_server();

    server.shutdown().await.unwrap();
}

#[test]
async fn test_did_open() {
    let server = create_server();
    let params = lsp::DidOpenTextDocumentParams {
        text_document: lsp::TextDocumentItem::new(
            lsp::Url::parse("file:///home/user/file.flux").unwrap(),
            "flux".to_string(),
            1,
            "from(".to_string(),
        ),
    };

    server.did_open(params).await;

    let uri = lsp::Url::parse("file:///home/user/file.flux").unwrap();
    let contents = server.store.get(&uri).unwrap();
    assert_eq!("from(", contents);
}

#[test]
async fn test_did_change() {
    let server = create_server();
    open_file(
        &server,
        r#"from(bucket: "bucket") |> first()"#.to_string(),
        None,
    )
    .await;

    let params = lsp::DidChangeTextDocumentParams {
        text_document: lsp::VersionedTextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
            version: -2,
        },
        content_changes: vec![lsp::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: r#"from(bucket: "bucket")"#.to_string(),
        }],
    };

    server.did_change(params).await;

    let uri = lsp::Url::parse("file:///home/user/file.flux").unwrap();
    let contents = server.store.get(&uri).unwrap();
    assert_eq!(r#"from(bucket: "bucket")"#, contents);
}

#[test]
async fn test_did_change_with_range() {
    let server = create_server();
    open_file(
        &server,
        r#"from(bucket: "bucket")
|> last()"#
            .to_string(),
        None,
    )
    .await;

    let params = lsp::DidChangeTextDocumentParams {
        text_document: lsp::VersionedTextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
            version: -2,
        },
        content_changes: vec![lsp::TextDocumentContentChangeEvent {
            range: Some(lsp::Range {
                start: lsp::Position {
                    line: 1,
                    character: 3,
                },
                end: lsp::Position {
                    line: 1,
                    character: 8,
                },
            }),
            range_length: None,
            text: r#" first()"#.to_string(),
        }],
    };

    server.did_change(params).await;

    let uri = lsp::Url::parse("file:///home/user/file.flux").unwrap();
    let contents = server.store.get(&uri).unwrap();
    assert_eq!(
        r#"from(bucket: "bucket")
|>  first()"#,
        contents
    );
}

#[test]
async fn test_did_change_with_multiline_range() {
    let server = create_server();
    open_file(
        &server,
        r#"from(bucket: "bucket")
|> group()
|> last()"#
            .to_string(),
        None,
    )
    .await;

    let params = lsp::DidChangeTextDocumentParams {
        text_document: lsp::VersionedTextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
            version: -2,
        },
        content_changes: vec![lsp::TextDocumentContentChangeEvent {
            range: Some(lsp::Range {
                start: lsp::Position {
                    line: 1,
                    character: 2,
                },
                end: lsp::Position {
                    line: 2,
                    character: 7,
                },
            }),
            range_length: None,
            text: r#"drop(columns: ["_start", "_stop"])
|>  first( "#
                .to_string(),
        }],
    };

    server.did_change(params).await;

    let uri = lsp::Url::parse("file:///home/user/file.flux").unwrap();
    let contents = server.store.get(&uri).unwrap();
    assert_eq!(
        r#"from(bucket: "bucket")
|>drop(columns: ["_start", "_stop"])
|>  first( )"#,
        contents
    );
}

#[test]
async fn test_did_save() {
    let server = create_server();
    open_file(
        &server,
        r#"from(bucket: "test") |> count()"#.to_string(),
        None,
    )
    .await;

    let uri = lsp::Url::parse("file:///home/user/file.flux").unwrap();

    let params = lsp::DidSaveTextDocumentParams {
        text_document: lsp::TextDocumentIdentifier::new(uri.clone()),
        text: Some(r#"from(bucket: "test2")"#.to_string()),
    };
    server.did_save(params).await;

    let contents = server.store.get(&uri).unwrap();
    assert_eq!(r#"from(bucket: "test2")"#.to_string(), contents);
}

#[test]
async fn test_did_close() {
    let server = create_server();
    open_file(&server, "from(".to_string(), None).await;

    assert!(server
        .store
        .get(&lsp::Url::parse("file:///home/user/file.flux").unwrap())
        .is_ok());

    let params = lsp::DidCloseTextDocumentParams {
        text_document: lsp::TextDocumentIdentifier::new(
            lsp::Url::parse("file:///home/user/file.flux").unwrap(),
        ),
    };

    server.did_close(params).await;

    assert!(server
        .store
        .get(&lsp::Url::parse("file:///home/user/file.flux").unwrap())
        .is_err());
}

// If the file hasn't been opened on the server get, return an error.
#[test]
async fn test_signature_help_not_opened() {
    let server = create_server();

    let params = lsp::SignatureHelpParams {
        context: None,
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                lsp::Position::new(0, 0),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };

    let result = server.signature_help(params).await;

    assert!(result.is_err());
}

#[test]
async fn test_signature_help() {
    let server = create_server();
    let fluxscript = r#"from(
                          // ^"#;
    open_file(&server, fluxscript.into(), None).await;

    let params = lsp::SignatureHelpParams {
        context: None,
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                position_of(fluxscript),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };

    let result =
        server.signature_help(params).await.unwrap().unwrap();

    let expected_signature_labels: Vec<String> = vec![
            "from()",
            "from(bucket: $bucket)",
            "from(bucketID: $bucketID)",
            "from(host: $host)",
            "from(org: $org)",
            "from(orgID: $orgID)",
            "from(token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID)",
            "from(bucket: $bucket , host: $host)",
            "from(bucket: $bucket , org: $org)",
            "from(bucket: $bucket , orgID: $orgID)",
            "from(bucket: $bucket , token: $token)",
            "from(bucketID: $bucketID , host: $host)",
            "from(bucketID: $bucketID , org: $org)",
            "from(bucketID: $bucketID , orgID: $orgID)",
            "from(bucketID: $bucketID , token: $token)",
            "from(host: $host , org: $org)",
            "from(host: $host , orgID: $orgID)",
            "from(host: $host , token: $token)",
            "from(org: $org , orgID: $orgID)",
            "from(org: $org , token: $token)",
            "from(orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org)",
            "from(bucket: $bucket , bucketID: $bucketID , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , token: $token)",
            "from(bucket: $bucket , host: $host , org: $org)",
            "from(bucket: $bucket , host: $host , orgID: $orgID)",
            "from(bucket: $bucket , host: $host , token: $token)",
            "from(bucket: $bucket , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , org: $org , token: $token)",
            "from(bucket: $bucket , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , host: $host , org: $org)",
            "from(bucketID: $bucketID , host: $host , orgID: $orgID)",
            "from(bucketID: $bucketID , host: $host , token: $token)",
            "from(bucketID: $bucketID , org: $org , orgID: $orgID)",
            "from(bucketID: $bucketID , org: $org , token: $token)",
            "from(bucketID: $bucketID , orgID: $orgID , token: $token)",
            "from(host: $host , org: $org , orgID: $orgID)",
            "from(host: $host , org: $org , token: $token)",
            "from(host: $host , orgID: $orgID , token: $token)",
            "from(org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , host: $host , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , host: $host , org: $org , token: $token)",
            "from(bucket: $bucket , host: $host , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , org: $org , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , host: $host , org: $org , orgID: $orgID)",
            "from(bucketID: $bucketID , host: $host , org: $org , token: $token)",
            "from(bucketID: $bucketID , host: $host , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , org: $org , orgID: $orgID , token: $token)",
            "from(host: $host , org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org , orgID: $orgID)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , host: $host , org: $org , orgID: $orgID , token: $token)",
            "from(bucketID: $bucketID , host: $host , org: $org , orgID: $orgID , token: $token)",
            "from(bucket: $bucket , bucketID: $bucketID , host: $host , org: $org , orgID: $orgID , token: $token)",
        ].into_iter().map(|x| x.into()).collect::<Vec<String>>();

    assert_eq!(
        expected_signature_labels,
        result
            .signatures
            .iter()
            .map(|x| x.label.clone())
            .collect::<Vec<String>>()
    );
    assert_eq!(None, result.active_signature);
    assert_eq!(None, result.active_parameter);
}

// If the file hasn't been opened on the server, return an error.
#[test]
async fn test_formatting_not_opened() {
    let server = create_server();

    let params = lsp::DocumentFormattingParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file::///home/user/file.flux")
                .unwrap(),
        },
        options: lsp::FormattingOptions {
            tab_size: 0,
            insert_spaces: false,
            properties:
                HashMap::<String, lsp::FormattingProperty>::new(),
            trim_trailing_whitespace: None,
            insert_final_newline: None,
            trim_final_newlines: None,
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };
    let result = server.formatting(params).await;

    assert!(result.is_err());
}

#[test]
async fn test_formatting() {
    let fluxscript = r#"
import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
      |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
      |> group(columns:["env", "error"])
    |> count()
  |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::DocumentFormattingParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        options: lsp::FormattingOptions {
            tab_size: 0,
            insert_spaces: false,
            properties:
                HashMap::<String, lsp::FormattingProperty>::new(),
            trim_trailing_whitespace: None,
            insert_final_newline: None,
            trim_final_newlines: None,
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };
    let result = server.formatting(params).await.unwrap().unwrap();

    let expected = lsp::TextEdit::new(
        lsp::Range {
            start: lsp::Position {
                line: 0,
                character: 0,
            },
            end: lsp::Position {
                line: 15,
                character: 96,
            },
        },
        flux::formatter::format(fluxscript).unwrap(),
    );
    assert_eq!(vec![expected], result);
}

#[test]
async fn test_formatting_insert_final_newline() {
    let fluxscript = r#"
import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
      |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
      |> group(columns:["env", "error"])
    |> count()
  |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))

"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::DocumentFormattingParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        options: lsp::FormattingOptions {
            tab_size: 0,
            insert_spaces: false,
            properties:
                HashMap::<String, lsp::FormattingProperty>::new(),
            trim_trailing_whitespace: None,
            insert_final_newline: Some(true),
            trim_final_newlines: None,
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };
    let result = server.formatting(params).await.unwrap().unwrap();

    let mut formatted_text =
        flux::formatter::format(fluxscript).unwrap();
    formatted_text.push('\n');
    let expected = lsp::TextEdit::new(
        lsp::Range {
            start: lsp::Position {
                line: 0,
                character: 0,
            },
            end: lsp::Position {
                line: 17,
                character: 0,
            },
        },
        formatted_text,
    );
    assert_eq!(vec![expected], result);
}

#[test]
async fn test_folding_not_opened() {
    let server = create_server();

    let params = lsp::FoldingRangeParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.folding_range(params).await;

    assert!(result.is_err());
}

#[test]
async fn test_folding() {
    let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::FoldingRangeParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.folding_range(params).await.unwrap().unwrap();

    let expected = vec![
        lsp::FoldingRange {
            start_line: 6,
            start_character: Some(26),
            end_line: 9,
            end_character: Some(38),
            kind: Some(lsp::FoldingRangeKind::Region),
        },
        lsp::FoldingRange {
            start_line: 15,
            start_character: Some(26),
            end_line: 15,
            end_character: Some(96),
            kind: Some(lsp::FoldingRangeKind::Region),
        },
    ];

    assert_eq!(expected, result);
}

#[test]
async fn test_document_symbol_not_opened() {
    let server = create_server();

    let params = lsp::DocumentSymbolParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.document_symbol(params).await;

    assert!(result.is_err());
}

#[test]
async fn test_document_symbol() {
    let expected_symbol_names: Vec<String> = vec![
        "strings",
        "env",
        "prod01-us-west-2",
        "errorCounts",
        "from",
        "bucket",
        "kube-infra/monthly",
        "range",
        "start",
        "filter",
        "fn",
        "r._measurement",
        "query_log",
        "r.error",
        "",
        "r._field",
        "responseSize",
        "r.env",
        "env",
        "group",
        "columns",
        "[]",
        "env",
        "error",
        "count",
        "group",
        "columns",
        "[]",
        "env",
        "_stop",
        "_start",
        "filter",
        "fn",
        "strings.containsStr",
        "v",
        "r.error",
        "substr",
        "AppendMappedRecordWithNulls",
    ]
    .into_iter()
    .map(|x| x.into())
    .collect::<Vec<String>>();

    let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::DocumentSymbolParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };
    let symbol_response =
        server.document_symbol(params).await.unwrap().unwrap();

    match symbol_response {
        lsp::DocumentSymbolResponse::Flat(symbols) => {
            assert_eq!(
                expected_symbol_names,
                symbols
                    .iter()
                    .map(|x| x.name.clone())
                    .collect::<Vec<String>>()
            );
        }
        _ => unreachable!(),
    }
}

#[test]
async fn test_goto_definition_not_opened() {
    let server = create_server();

    let params = lsp::GotoDefinitionParams {
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                lsp::Position::new(1, 1),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.goto_definition(params).await;

    assert!(result.is_err());
}

#[test]
async fn test_goto_definition() {
    let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
                                // ^
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::GotoDefinitionParams {
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                position_of(fluxscript),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result =
        server.goto_definition(params).await.unwrap().unwrap();

    let expected =
        lsp::GotoDefinitionResponse::Scalar(lsp::Location {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
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
        });

    assert_eq!(expected, result);
}

#[test]
async fn test_goto_definition_builtin() {
    let fluxscript = r#"
builtin func : (x: A) => A

func(x: 1)
// ^
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::GotoDefinitionParams {
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                position_of(fluxscript),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.goto_definition(params).await.unwrap();

    expect_test::expect![[r#"
            {
              "uri": "file:///home/user/file.flux",
              "range": {
                "start": {
                  "line": 1,
                  "character": 8
                },
                "end": {
                  "line": 1,
                  "character": 12
                }
              }
            }"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

#[test]
async fn test_goto_definition_shadowed() {
    let fluxscript = r#"
env = "prod01-us-west-2"

f = (env) => {
    return env
        // ^
}
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::GotoDefinitionParams {
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                position_of(fluxscript),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.goto_definition(params).await.unwrap();

    expect![[r#"
            {
              "uri": "file:///home/user/file.flux",
              "range": {
                "start": {
                  "line": 3,
                  "character": 5
                },
                "end": {
                  "line": 3,
                  "character": 8
                }
              }
            }"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

#[test]
async fn test_rename() {
    let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::RenameParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 1,
                character: 1,
            },
        },
        new_name: "environment".to_string(),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };

    let result = server.rename(params).await.unwrap().unwrap();

    let edits = vec![
        lsp::TextEdit {
            new_text: "environment".to_string(),
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
            new_text: "environment".to_string(),
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
    let mut changes: HashMap<lsp::Url, Vec<lsp::TextEdit>> =
        HashMap::new();
    changes.insert(
        lsp::Url::parse("file:///home/user/file.flux").unwrap(),
        edits,
    );

    let expected = lsp::WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    };

    assert_eq!(expected, result);
}

#[test]
async fn test_references() {
    let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::ReferenceParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 1,
                character: 1,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: lsp::ReferenceContext {
            // declaration is included whether this is true or false
            include_declaration: true,
        },
    };

    let result =
        server.references(params.clone()).await.unwrap().unwrap();

    let expected = vec![
        lsp::Location {
            uri: params
                .text_document_position
                .text_document
                .uri
                .clone(),
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
            uri: params
                .text_document_position
                .text_document
                .uri
                .clone(),
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

    assert_eq!(expected, result);
}

#[test]
async fn test_references_duplicates() {
    let fluxscript = r#"
t = (x) => {
    x = 1
    return x
        // ^
}"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::ReferenceParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: position_of(fluxscript),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: lsp::ReferenceContext {
            // declaration is included whether this is true or false
            include_declaration: true,
        },
    };

    let result =
        server.references(params.clone()).await.unwrap().unwrap();

    expect![[r#"
            [
              {
                "uri": "file:///home/user/file.flux",
                "range": {
                  "start": {
                    "line": 2,
                    "character": 4
                  },
                  "end": {
                    "line": 2,
                    "character": 5
                  }
                }
              },
              {
                "uri": "file:///home/user/file.flux",
                "range": {
                  "start": {
                    "line": 3,
                    "character": 11
                  },
                  "end": {
                    "line": 3,
                    "character": 12
                  }
                }
              }
            ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

#[test]
async fn test_document_highlight() {
    let fluxscript = r#"import "strings"
env = "prod01-us-west-2"

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::DocumentHighlightParams {
        text_document_position_params:
            lsp::TextDocumentPositionParams {
                text_document: lsp::TextDocumentIdentifier {
                    uri: lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lsp::Position {
                    line: 1,
                    character: 1,
                },
            },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server
        .document_highlight(params.clone())
        .await
        .unwrap()
        .unwrap();

    let expected = vec![
        lsp::DocumentHighlight {
            kind: Some(lsp::DocumentHighlightKind::TEXT),
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
        lsp::DocumentHighlight {
            kind: Some(lsp::DocumentHighlightKind::TEXT),
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

    assert_eq!(expected, result);
}

fn hover_params(pos: lsp::Position) -> lsp::HoverParams {
    lsp::HoverParams {
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                pos,
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    }
}

#[test]
async fn test_hover() {
    let fluxscript = r#"x = 1
x + 1
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = hover_params(lsp::Position::new(1, 1));

    let result = server.hover(params).await.unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String("type: int".to_string())
            ),
            range: None,
        })
    );
}

#[test]
async fn test_hover_binding() {
    let fluxscript = r#"x = "asd"
builtin builtin_ : (v: int) => int
option option_ = 123
1
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let result = server
        .hover(hover_params(lsp::Position::new(0, 1)))
        .await
        .unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String("type: string".to_string())
            ),
            range: None,
        })
    );

    let result = server
        .hover(hover_params(lsp::Position::new(1, 12)))
        .await
        .unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String(
                    "type: (v:int) => int".to_string()
                )
            ),
            range: None,
        })
    );

    let result = server
        .hover(hover_params(lsp::Position::new(2, 10)))
        .await
        .unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String("type: int".to_string())
            ),
            range: None,
        })
    );
}

#[test]
async fn test_hover_argument() {
    let fluxscript = r#"
(x) => x + 1
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = hover_params(lsp::Position::new(1, 1));

    let result = server.hover(params).await.unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String("type: int".to_string())
            ),
            range: None,
        })
    );
}

#[test]
async fn test_hover_call_property() {
    let fluxscript = r#"
f = (x) => x + 1
y = f(x: 1)
   // ^
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = hover_params(position_of(fluxscript));

    let result = server.hover(params).await.unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String("type: int".to_string())
            ),
            range: None,
        })
    );
}

#[test]
async fn test_hover_record_property() {
    let fluxscript = r#"
{ abc: "" }
// ^
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = hover_params(position_of(fluxscript));

    let result = server.hover(params).await.unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String("type: string".to_string())
            ),
            range: None,
        })
    );
}

#[test]
async fn test_hover_on_semantic_error() {
    let fluxscript = r#"
y = 1 + ""
x = 1
1 + x
 // ^
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = hover_params(position_of(fluxscript));

    let result = server.hover(params).await.unwrap();

    assert_eq!(
        result,
        Some(lsp::Hover {
            contents: lsp::HoverContents::Scalar(
                lsp::MarkedString::String("type: int".to_string())
            ),
            range: None,
        })
    );
}

#[test]
async fn test_package_completion() {
    let fluxscript = r#"import "sql"

sql.
// ^
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: position_of(fluxscript),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind:
                lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some(".".to_string()),
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let expected_labels: Vec<String> = vec!["to", "from"]
        .into_iter()
        .map(|x| x.into())
        .collect::<Vec<String>>();

    match result {
        lsp::CompletionResponse::List(l) => {
            assert_eq!(
                expected_labels,
                l.items
                    .iter()
                    .map(|x| x.label.clone())
                    .collect::<Vec<String>>()
            );
        }
        _ => unreachable!(),
    };
}

#[test]
async fn test_import_completion() {
    let fluxscript = r#"
import "
    // ^

x = 1
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: position_of(fluxscript),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    match result {
        lsp::CompletionResponse::List(l) => {
            expect![[r#"
                [
                    "\"array\"",
                    "\"contrib/RohanSreerama5/naiveBayesClassifier\"",
                    "\"contrib/anaisdg/anomalydetection\"",
                    "\"contrib/anaisdg/statsmodels\"",
                    "\"contrib/bonitoo-io/alerta\"",
                    "\"contrib/bonitoo-io/hex\"",
                    "\"contrib/bonitoo-io/servicenow\"",
                    "\"contrib/bonitoo-io/tickscript\"",
                    "\"contrib/bonitoo-io/victorops\"",
                    "\"contrib/bonitoo-io/zenoss\"",
                    "\"contrib/chobbs/discord\"",
                    "\"contrib/jsternberg/aggregate\"",
                    "\"contrib/jsternberg/influxdb\"",
                    "\"contrib/jsternberg/math\"",
                    "\"contrib/rhajek/bigpanda\"",
                    "\"contrib/sranka/opsgenie\"",
                    "\"contrib/sranka/sensu\"",
                    "\"contrib/sranka/teams\"",
                    "\"contrib/sranka/telegram\"",
                    "\"contrib/sranka/webexteams\"",
                    "\"contrib/tomhollingworth/events\"",
                    "\"csv\"",
                    "\"date\"",
                    "\"dict\"",
                    "\"experimental\"",
                    "\"experimental/aggregate\"",
                    "\"experimental/array\"",
                    "\"experimental/bigtable\"",
                    "\"experimental/bitwise\"",
                    "\"experimental/csv\"",
                    "\"experimental/geo\"",
                    "\"experimental/http\"",
                    "\"experimental/http/requests\"",
                    "\"experimental/influxdb\"",
                    "\"experimental/iox\"",
                    "\"experimental/json\"",
                    "\"experimental/mqtt\"",
                    "\"experimental/oee\"",
                    "\"experimental/prometheus\"",
                    "\"experimental/query\"",
                    "\"experimental/record\"",
                    "\"experimental/table\"",
                    "\"experimental/universe\"",
                    "\"experimental/usage\"",
                    "\"generate\"",
                    "\"http\"",
                    "\"influxdata/influxdb\"",
                    "\"influxdata/influxdb/monitor\"",
                    "\"influxdata/influxdb/sample\"",
                    "\"influxdata/influxdb/schema\"",
                    "\"influxdata/influxdb/secrets\"",
                    "\"influxdata/influxdb/tasks\"",
                    "\"influxdata/influxdb/v1\"",
                    "\"internal/boolean\"",
                    "\"internal/debug\"",
                    "\"internal/gen\"",
                    "\"internal/influxql\"",
                    "\"internal/location\"",
                    "\"internal/promql\"",
                    "\"internal/testutil\"",
                    "\"interpolate\"",
                    "\"join\"",
                    "\"json\"",
                    "\"kafka\"",
                    "\"math\"",
                    "\"pagerduty\"",
                    "\"planner\"",
                    "\"profiler\"",
                    "\"pushbullet\"",
                    "\"regexp\"",
                    "\"runtime\"",
                    "\"sampledata\"",
                    "\"slack\"",
                    "\"socket\"",
                    "\"sql\"",
                    "\"strings\"",
                    "\"system\"",
                    "\"testing\"",
                    "\"testing/expect\"",
                    "\"timezone\"",
                    "\"types\"",
                    "\"universe\"",
                ]
            "#]]
            .assert_debug_eq(
                &l.items.iter().map(|x| &x.label).collect::<Vec<_>>(),
            );
        }

        _ => unreachable!(),
    };
}

#[test]
async fn test_variable_completion() {
    let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

c

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d)
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 8,
                character: 1,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let items = match result {
        lsp::CompletionResponse::List(l) => l.items,
        _ => unreachable!(),
    };

    let got: BTreeSet<&str> =
        items.iter().map(|i| i.label.as_str()).collect();

    let want: BTreeSet<&str> = vec![
        "buckets",
        "cardinality",
        "chandeMomentumOscillator",
        "columns",
        "contains",
        "contrib/RohanSreerama5/naiveBayesClassifier",
        "contrib/anaisdg/anomalydetection",
        "contrib/bonitoo-io/servicenow",
        "contrib/bonitoo-io/tickscript",
        "contrib/bonitoo-io/victorops",
        "contrib/chobbs/discord",
        "count",
        "cov",
        "covariance",
        "csv",
        "cumulativeSum",
        "dict",
        "difference",
        "distinct",
        "duplicate",
        "experimental/csv",
        "experimental/record",
        "findColumn",
        "findRecord",
        "getColumn",
        "getRecord",
        "highestCurrent",
        "hourSelection",
        "increase",
        "influxdata/influxdb/schema",
        "influxdata/influxdb/secrets",
        "internal/location",
        "logarithmicBins",
        "lowestCurrent",
        "reduce",
        "slack",
        "socket",
        "stateCount",
        "stateTracking",
        "testing/expect",
        "truncateTimeColumn",
    ]
    .drain(..)
    .collect();

    assert_eq!(
        want,
        got,
        "\nextra:\n {:?}\n missing:\n {:?}\n",
        got.difference(&want),
        want.difference(&got)
    );
}

#[test]
async fn test_option_object_members_completion() {
    let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

option task = {
  name: "foo",        // Name is required.
  every: 1h,          // Task should be run at this interval.
  delay: 10m,         // Delay scheduling this task by this duration.
  cron: "0 2 * * *",  // Cron is a more sophisticated way to schedule. 'every' and 'cron' are mutually exclusive.
  retry: 5,           // Number of times to retry a failed query.
}

task.
 // ^

ab = 10
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: position_of(fluxscript),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind:
                lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some(".".to_string()),
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let items = match result {
        lsp::CompletionResponse::List(l) => l.items,
        _ => unreachable!(),
    };

    let labels: Vec<&str> =
        items.iter().map(|item| item.label.as_str()).collect();

    let expected = vec![
        "name (self)",
        "every (self)",
        "delay (self)",
        "cron (self)",
        "retry (self)",
    ];

    assert_eq!(expected, labels);
}

#[test]
async fn test_option_function_completion() {
    let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

option now = () => 2020-02-20T23:00:00Z

n

ab = 10
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 10,
                character: 1,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let items = match result {
        lsp::CompletionResponse::List(l) => l.items,
        _ => unreachable!(),
    };

    let got: BTreeSet<&str> =
        items.iter().map(|i| i.label.as_str()).collect();

    expect![[r#"
        [
          "aggregateWindow",
          "cardinality",
          "chandeMomentumOscillator",
          "columns",
          "contains",
          "contrib/RohanSreerama5/naiveBayesClassifier",
          "contrib/anaisdg/anomalydetection",
          "contrib/bonitoo-io/servicenow",
          "contrib/bonitoo-io/zenoss",
          "contrib/jsternberg/influxdb",
          "contrib/rhajek/bigpanda",
          "contrib/sranka/opsgenie",
          "contrib/sranka/sensu",
          "contrib/tomhollingworth/events",
          "count",
          "covariance",
          "difference",
          "distinct",
          "duration",
          "experimental",
          "experimental/influxdb",
          "experimental/json",
          "experimental/universe",
          "exponentialMovingAverage",
          "findColumn",
          "findRecord",
          "generate",
          "getColumn",
          "highestCurrent",
          "histogramQuantile",
          "holtWinters",
          "hourSelection",
          "increase",
          "inf (prelude)",
          "influxdata/influxdb",
          "influxdata/influxdb/monitor",
          "int",
          "integral",
          "internal/boolean",
          "internal/gen",
          "internal/influxql",
          "internal/location",
          "interpolate",
          "join",
          "json",
          "kaufmansAMA",
          "kaufmansER",
          "length",
          "linearBins",
          "logarithmicBins",
          "lowestCurrent",
          "lowestMin",
          "mean",
          "median",
          "min",
          "movingAverage",
          "now",
          "pearsonr",
          "planner",
          "quantile",
          "range",
          "relativeStrengthIndex",
          "rename",
          "runtime",
          "stateCount",
          "stateDuration",
          "stateTracking",
          "string",
          "strings",
          "tableFind",
          "testing",
          "timedMovingAverage",
          "timezone",
          "toInt",
          "toString",
          "toUInt",
          "tripleExponentialDerivative",
          "truncateTimeColumn",
          "uint",
          "union",
          "unique",
          "universe",
          "window"
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&got).unwrap());
}

#[test]
async fn test_object_param_completion() {
    let fluxscript = r#"obj = {
    func: (name, age) => name + age
}

obj.func(
     // ^
        "#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: position_of(fluxscript),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind:
                lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some("(".to_string()),
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let items = match result {
        lsp::CompletionResponse::List(l) => l.items,
        _ => unreachable!(),
    };

    let labels: Vec<&str> =
        items.iter().map(|item| item.label.as_str()).collect();

    let expected = vec!["name", "age"];

    assert_eq!(expected, labels);
}

#[test]
async fn test_param_completion() {
    let fluxscript = r#"import "csv"

csv.from(
     // ^
        "#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: position_of(fluxscript),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind:
                lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some("(".to_string()),
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    expect![[r#"
        {
          "isIncomplete": false,
          "items": [
            {
              "label": "csv",
              "kind": 5,
              "detail": "string",
              "insertText": "csv: ",
              "insertTextFormat": 2
            },
            {
              "label": "file",
              "kind": 5,
              "detail": "string",
              "insertText": "file: ",
              "insertTextFormat": 2
            },
            {
              "label": "mode",
              "kind": 5,
              "detail": "string",
              "insertText": "mode: ",
              "insertTextFormat": 2
            },
            {
              "label": "url",
              "kind": 5,
              "detail": "string",
              "insertText": "url: ",
              "insertTextFormat": 2
            }
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

#[test]
async fn test_param_completion_2() {
    let fluxscript = r#"import "csv"

csv.from(
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 3,
                character: 0,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let items = match result {
        lsp::CompletionResponse::List(l) => l.items,
        _ => unreachable!(),
    };

    let labels: Vec<&str> =
        items.iter().map(|item| item.label.as_str()).collect();

    let expected = vec!["csv", "file", "mode", "url"];

    assert_eq!(expected, labels);
}

#[test]
async fn test_param_completion_3() {
    let fluxscript = r#"import "csv"

csv.from(mode: "raw",
                   // ^
x = 1
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: position_of(fluxscript),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let items = match result {
        lsp::CompletionResponse::List(l) => l.items,
        _ => unreachable!(),
    };

    let labels: Vec<&str> =
        items.iter().map(|item| item.label.as_str()).collect();

    let expected = vec!["csv", "file", "url"];

    assert_eq!(expected, labels);
}

#[test]
async fn test_options_completion() {
    let fluxscript = r#"import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

option task = {
  name: "foo",        // Name is required.
  every: 1h,          // Task should be run at this interval.
  delay: 10m,         // Delay scheduling this task by this duration.
  cron: "0 2 * * *",  // Cron is a more sophisticated way to schedule. 'every' and 'cron' are mutually exclusive.
  retry: 5,           // Number of times to retry a failed query.
}

newNow = t

errorCounts = from(bucket:"kube-infra/monthly")
    |> range(start: -3d )
    |> filter(fn: (r) => r._measurement == "query_log" and
                         r.error != "" and
                         r._field == "responseSize" and
                         r.env == env)
    |> group(columns:["env", "error"])
    |> count()
    |> group(columns:["env", "_stop", "_start"])

errorCounts
    |> filter(fn: (r) => strings.containsStr(v: r.error, substr: "AppendMappedRecordWithNulls"))

"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 16,
                character: 10,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    let items = match result {
        lsp::CompletionResponse::List(l) => l.items,
        _ => unreachable!(),
    };

    let got: BTreeSet<&str> =
        items.iter().map(|i| i.label.as_str()).collect();

    let want: BTreeSet<&str> = vec![
        "aggregateWindow",
        "bottom",
        "buckets",
        "bytes",
        "cardinality",
        "chandeMomentumOscillator",
        "contains",
        "contrib/anaisdg/anomalydetection",
        "contrib/anaisdg/statsmodels",
        "contrib/bonitoo-io/alerta",
        "contrib/bonitoo-io/tickscript",
        "contrib/bonitoo-io/victorops",
        "contrib/jsternberg/aggregate",
        "contrib/jsternberg/math",
        "contrib/sranka/teams",
        "contrib/sranka/telegram",
        "contrib/sranka/webexteams",
        "contrib/tomhollingworth/events",
        "count",
        "cumulativeSum",
        "date",
        "derivative",
        "dict",
        "distinct",
        "duplicate",
        "duration",
        "experimental",
        "experimental/aggregate",
        "experimental/bigtable",
        "experimental/bitwise",
        "experimental/http",
        "experimental/http/requests",
        "experimental/mqtt",
        "experimental/prometheus",
        "experimental/table",
        "exponentialMovingAverage",
        "filter",
        "first",
        "float",
        "generate",
        "getColumn",
        "getRecord",
        "highestAverage",
        "highestCurrent",
        "highestMax",
        "histogram",
        "histogramQuantile",
        "holtWinters",
        "hourSelection",
        "http",
        "influxdata/influxdb/monitor",
        "influxdata/influxdb/secrets",
        "influxdata/influxdb/tasks",
        "int",
        "integral",
        "internal/testutil",
        "internal/location",
        "interpolate",
        "last",
        "length",
        "limit",
        "logarithmicBins",
        "lowestAverage",
        "lowestCurrent",
        "lowestMin",
        "math",
        "pagerduty",
        "pivot",
        "pushbullet",
        "quantile",
        "relativeStrengthIndex",
        "runtime",
        "sampledata",
        "set",
        "socket",
        "sort",
        "stateCount",
        "stateDuration",
        "stateTracking",
        "stddev",
        "string",
        "strings",
        "system",
        "tableFind",
        "tail",
        "testing",
        "testing/expect",
        "time",
        "timeShift",
        "timeWeightedAvg",
        "timedMovingAverage",
        "timezone",
        "to",
        "toBool",
        "toFloat",
        "toInt",
        "toString",
        "toTime",
        "toUInt",
        "today",
        "top",
        "tripleEMA",
        "tripleExponentialDerivative",
        "true (prelude)",
        "truncateTimeColumn",
        "types",
        "uint",
    ]
    .drain(..)
    .collect();

    assert_eq!(
        want,
        got,
        "\nextra:\n {:?}\n missing:\n {:?}\n",
        got.difference(&want),
        want.difference(&got)
    );
}

#[test]
async fn test_signature_help_invalid() {
    let fluxscript = r#"bork |>"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::SignatureHelpParams {
        context: None,
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                lsp::Position::new(0, 5),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };

    let result = server.signature_help(params).await;
    assert_eq!(result, Ok(None));
}

#[test]
async fn test_folding_range_invalid() {
    let fluxscript = r#"bork |>"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::FoldingRangeParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.folding_range(params).await;
    assert_eq!(result, Ok(None));
}

#[test]
async fn test_document_symbol_invalid() {
    let fluxscript = r#"bork |>"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::DocumentSymbolParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };
    let result = server.document_symbol(params).await;

    assert_eq!(result, Ok(None));
}

#[test]
async fn test_goto_definition_invalid() {
    let fluxscript = r#"bork |>"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::GotoDefinitionParams {
        text_document_position_params:
            lsp::TextDocumentPositionParams::new(
                lsp::TextDocumentIdentifier::new(
                    lsp::Url::parse("file:///home/user/file.flux")
                        .unwrap(),
                ),
                lsp::Position::new(8, 35),
            ),
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.goto_definition(params).await;
    assert!(matches!(result, Ok(None)));
}

#[test]
async fn test_rename_invalid() {
    let fluxscript = r#"bork |>"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::ReferenceParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 1,
                character: 1,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: lsp::ReferenceContext {
            // declaration is included whether this is true or false
            include_declaration: true,
        },
    };

    let result = server.references(params.clone()).await;

    assert_eq!(result, Ok(None));
}

// Historically, the completion of a package also brought with it an additional edit
// that would import the stdlib module it was referring to. This test asserts that we
// don't add it back in, but also serves as documentation that this was a conscious
// choice, as the user experience was not good.
#[test]
async fn test_package_completion_when_it_is_not_imported() {
    let fluxscript = r#"sql"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 0,
                character: 2,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    expect_test::expect![[r#"
                    {
                      "isIncomplete": false,
                      "items": [
                        {
                          "label": "sql",
                          "kind": 9,
                          "detail": "Package",
                          "documentation": "sql",
                          "sortText": "sql",
                          "filterText": "sql",
                          "insertText": "sql",
                          "insertTextFormat": 1
                        }
                      ]
                    }"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

#[test]
async fn test_package_completion_when_it_is_imported() {
    let fluxscript = r#"import "sql"

sql"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CompletionParams {
        text_document_position: lsp::TextDocumentPositionParams {
            text_document: lsp::TextDocumentIdentifier {
                uri: lsp::Url::parse("file:///home/user/file.flux")
                    .unwrap(),
            },
            position: lsp::Position {
                line: 2,
                character: 2,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lsp::CompletionContext {
            trigger_kind: lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    let result =
        server.completion(params.clone()).await.unwrap().unwrap();

    expect_test::expect![[r#"
            {
              "isIncomplete": false,
              "items": [
                {
                  "label": "sql",
                  "kind": 9,
                  "detail": "Package",
                  "documentation": "sql",
                  "sortText": "sql",
                  "filterText": "sql",
                  "insertText": "sql",
                  "insertTextFormat": 1
                }
              ]
            }"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

use crate::visitors::ast::{
    SEMANTIC_TOKEN_KEYWORD, SEMANTIC_TOKEN_NUMBER,
    SEMANTIC_TOKEN_STRING,
};

#[test]
async fn test_semantic_tokens_full() {
    let fluxscript = r#"package "my-package"
import "csv"

myVar = from(bucket: "my-bucket")
    |> range(start: 30m)

csv.from(file: "my.csv")
    |> filter(fn: (row) => row.field == 0.9)
"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::SemanticTokensParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.semantic_tokens_full(params).await.unwrap();
    assert!(result.is_some());

    let token_result = result.unwrap();
    if let lsp::SemanticTokensResult::Tokens(tokens) = token_result {
        let expected = lsp::SemanticTokens {
            result_id: None,
            data: vec![
                // package
                lsp::SemanticToken {
                    delta_line: 1,
                    delta_start: 1,
                    length: 7,
                    token_type: SEMANTIC_TOKEN_KEYWORD,
                    token_modifiers_bitset: 0,
                },
                // "my-package"
                lsp::SemanticToken {
                    delta_line: 1,
                    delta_start: 9,
                    length: 12,
                    token_type: SEMANTIC_TOKEN_STRING,
                    token_modifiers_bitset: 0,
                },
                // import
                lsp::SemanticToken {
                    delta_line: 2,
                    delta_start: 1,
                    length: 6,
                    token_type: SEMANTIC_TOKEN_KEYWORD,
                    token_modifiers_bitset: 0,
                },
                // "csv"
                lsp::SemanticToken {
                    delta_line: 2,
                    delta_start: 8,
                    length: 5,
                    token_type: SEMANTIC_TOKEN_STRING,
                    token_modifiers_bitset: 0,
                },
                // "my-bucket"
                lsp::SemanticToken {
                    delta_line: 4,
                    delta_start: 22,
                    length: 11,
                    token_type: SEMANTIC_TOKEN_STRING,
                    token_modifiers_bitset: 0,
                },
                // 30m
                lsp::SemanticToken {
                    delta_line: 5,
                    delta_start: 21,
                    length: 3,
                    token_type: SEMANTIC_TOKEN_NUMBER,
                    token_modifiers_bitset: 0,
                },
                // my.csv
                lsp::SemanticToken {
                    delta_line: 7,
                    delta_start: 16,
                    length: 8,
                    token_type: SEMANTIC_TOKEN_STRING,
                    token_modifiers_bitset: 0,
                },
                // 0.9
                lsp::SemanticToken {
                    delta_line: 8,
                    delta_start: 41,
                    length: 3,
                    token_type: SEMANTIC_TOKEN_NUMBER,
                    token_modifiers_bitset: 0,
                },
            ],
        };
        assert_eq!(expected, tokens)
    } else {
        panic!("Result was not a token result");
    }
}

// A code action for importing a package when an undefined identifier
// is found.
#[test]
async fn test_code_action_import_insertion() {
    let fluxscript = r#"sql"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CodeActionParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        context: lsp::CodeActionContext {
            diagnostics: vec![lsp::Diagnostic {
                code: None,
                code_description: None,
                data: None,
                related_information: None,
                severity: Some(lsp::DiagnosticSeverity::ERROR),
                source: Some("flux".into()),
                tags: None,
                message: "undefined identifier sql".into(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                },
            }],
            only: None,
        },
        range: lsp::Range {
            start: lsp::Position {
                line: 0,
                character: 2,
            },
            end: lsp::Position {
                line: 0,
                character: 2,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.code_action(params).await.unwrap();

    expect_test::expect![[r#"
            [
              {
                "title": "Import `sql`",
                "kind": "quickfix",
                "edit": {
                  "changes": {
                    "file:///home/user/file.flux": [
                      {
                        "range": {
                          "start": {
                            "line": 0,
                            "character": 0
                          },
                          "end": {
                            "line": 0,
                            "character": 0
                          }
                        },
                        "newText": "import \"sql\"\n"
                      }
                    ]
                  }
                },
                "isPreferred": true
              }
            ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

// When inserting a package import, don't clobber the package statement at the beginning.
#[test]
async fn test_code_action_import_insertion_with_package() {
    let fluxscript = "package anPackage\n\nsql";
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CodeActionParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        context: lsp::CodeActionContext {
            diagnostics: vec![lsp::Diagnostic {
                code: None,
                code_description: None,
                data: None,
                related_information: None,
                severity: Some(lsp::DiagnosticSeverity::ERROR),
                source: Some("flux".into()),
                tags: None,
                message: "undefined identifier sql".into(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 2,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 2,
                        character: 0,
                    },
                },
            }],
            only: None,
        },
        range: lsp::Range {
            start: lsp::Position {
                line: 2,
                character: 2,
            },
            end: lsp::Position {
                line: 2,
                character: 2,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.code_action(params).await.unwrap();

    expect_test::expect![[r#"
            [
              {
                "title": "Import `sql`",
                "kind": "quickfix",
                "edit": {
                  "changes": {
                    "file:///home/user/file.flux": [
                      {
                        "range": {
                          "start": {
                            "line": 1,
                            "character": 0
                          },
                          "end": {
                            "line": 1,
                            "character": 0
                          }
                        },
                        "newText": "import \"sql\"\n"
                      }
                    ]
                  }
                },
                "isPreferred": true
              }
            ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

/// If the identifier matches multiple potential imports, multiple code
/// actions should be offered to the user.
#[test]
async fn test_code_action_import_insertion_multiple_actions() {
    let fluxscript = r#"array"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CodeActionParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        context: lsp::CodeActionContext {
            diagnostics: vec![lsp::Diagnostic {
                code: None,
                code_description: None,
                data: None,
                related_information: None,
                severity: Some(lsp::DiagnosticSeverity::ERROR),
                source: Some("flux".into()),
                tags: None,
                message: "undefined identifier array".into(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                },
            }],
            only: None,
        },
        range: lsp::Range {
            start: lsp::Position {
                line: 0,
                character: 4,
            },
            end: lsp::Position {
                line: 0,
                character: 4,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.code_action(params).await.unwrap();

    expect_test::expect![[r#"
            [
              {
                "title": "Import `array`",
                "kind": "quickfix",
                "edit": {
                  "changes": {
                    "file:///home/user/file.flux": [
                      {
                        "range": {
                          "start": {
                            "line": 0,
                            "character": 0
                          },
                          "end": {
                            "line": 0,
                            "character": 0
                          }
                        },
                        "newText": "import \"array\"\n"
                      }
                    ]
                  }
                },
                "isPreferred": true
              },
              {
                "title": "Import `experimental/array`",
                "kind": "quickfix",
                "edit": {
                  "changes": {
                    "file:///home/user/file.flux": [
                      {
                        "range": {
                          "start": {
                            "line": 0,
                            "character": 0
                          },
                          "end": {
                            "line": 0,
                            "character": 0
                          }
                        },
                        "newText": "import \"experimental/array\"\n"
                      }
                    ]
                  }
                },
                "isPreferred": true
              }
            ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

/// If the import requires a full path, that the action suggests the full path.
#[test]
async fn test_code_action_import_insertion_full_path() {
    let fluxscript = r#"schema"#;
    let server = create_server();
    open_file(&server, fluxscript.to_string(), None).await;

    let params = lsp::CodeActionParams {
        text_document: lsp::TextDocumentIdentifier {
            uri: lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
        },
        context: lsp::CodeActionContext {
            diagnostics: vec![lsp::Diagnostic {
                code: None,
                code_description: None,
                data: None,
                related_information: None,
                severity: Some(lsp::DiagnosticSeverity::ERROR),
                source: Some("flux".into()),
                tags: None,
                message: "undefined identifier schema".into(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                },
            }],
            only: None,
        },
        range: lsp::Range {
            start: lsp::Position {
                line: 0,
                character: 5,
            },
            end: lsp::Position {
                line: 0,
                character: 5,
            },
        },
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: lsp::PartialResultParams {
            partial_result_token: None,
        },
    };

    let result = server.code_action(params).await.unwrap();

    expect_test::expect![[r#"
            [
              {
                "title": "Import `influxdata/influxdb/schema`",
                "kind": "quickfix",
                "edit": {
                  "changes": {
                    "file:///home/user/file.flux": [
                      {
                        "range": {
                          "start": {
                            "line": 0,
                            "character": 0
                          },
                          "end": {
                            "line": 0,
                            "character": 0
                          }
                        },
                        "newText": "import \"influxdata/influxdb/schema\"\n"
                      }
                    ]
                  }
                },
                "isPreferred": true
              }
            ]"#]]
        .assert_eq(&serde_json::to_string_pretty(&result).unwrap());
}

#[test]
async fn compute_diagnostics_multi_file() {
    let server = create_server();

    let filename: String = "file:///path/to/script.flux".into();
    let fluxscript = r#"from(bucket: "my-bucket")
|> range(start: -100d)
|> filter(fn: (r) => r.anTag == v.a)"#;
    open_file(&server, fluxscript.into(), Some(&filename)).await;

    let diagnostics = server
        .compute_diagnostics(&lsp::Url::parse(&filename).unwrap());

    assert_eq!(
        vec![lsp::Diagnostic {
            code: None,
            code_description: None,
            data: None,
            message: "undefined identifier v".into(),
            range: lsp::Range {
                start: lsp::Position {
                    line: 2,
                    character: 32
                },
                end: lsp::Position {
                    line: 2,
                    character: 33
                },
            },
            related_information: None,
            severity: Some(lsp::DiagnosticSeverity::ERROR),
            source: Some("flux".into()),
            tags: None,
        }],
        diagnostics
    );

    open_file(
        &server,
        r#"v = {a: "b"}"#.to_string(),
        Some("file:///path/to/an_vars.flux"),
    )
    .await;

    let diagnostics_again = server
        .compute_diagnostics(&lsp::Url::parse(&filename).unwrap());

    assert_eq!(0, diagnostics_again.len());
}

// Only emit diagnostics related to that specific file.
#[test]
async fn compute_diagnostics_only_on_problem_file() {
    let server = create_server();

    let filename: String = "file:///path/to/script.flux".into();
    let fluxscript = r#"from(bucket: "my-bucket")
|> range(start: -100d)
|> filter(fn: (r) => r.anTag == v.a)"#;
    open_file(&server, fluxscript.into(), Some(&filename)).await;
    // This file, in the same package, contains an error.
    open_file(
        &server,
        r#"v = a"#.to_string(),
        Some("file:///path/to/an_vars.flux"),
    )
    .await;

    let diagnostics_again = server
        .compute_diagnostics(&lsp::Url::parse(&filename).unwrap());

    assert!(diagnostics_again.is_empty());
}

#[test]
async fn compute_diagnostics_non_errors() {
    let server = create_server();

    let filename: String = "file:///path/to/script.flux".into();
    let fluxscript = r#"import "experimental"
        
from(bucket: "my-bucket")
|> range(start: -100d)
|> filter(fn: (r) => r.value == "b")
|> experimental.to(bucket: "out-bucket", org: "abc123", host: "https://myhost.example.com", token: "123abc")"#;
    open_file(&server, fluxscript.into(), Some(&filename)).await;

    let diagnostics_again = server
        .compute_diagnostics(&lsp::Url::parse(&filename).unwrap());

    assert!(!diagnostics_again.is_empty());
}

/// All commands require key/value pairs as params, not positional
/// arguments.
#[test]
async fn execute_command_too_many_args() {
    let server = create_server();
    let params = lsp::ExecuteCommandParams {
        command: "notActuallyAValidCommand".into(),
        arguments: vec![
            serde_json::value::to_value("arg1").unwrap(),
            serde_json::value::to_value("arg2").unwrap(),
        ],
        work_done_progress_params: lsp::WorkDoneProgressParams {
            work_done_token: None,
        },
    };

    let result = server.execute_command(params).await;

    assert!(result.is_err());
}
