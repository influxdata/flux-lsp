use async_std::task::block_on;
use criterion::{
    black_box, criterion_group, criterion_main, Criterion,
};
use flux_lsp::LspServer;
use lspower::LanguageServer;

fn create_server() -> LspServer {
    LspServer::new(None)
}

fn open_file(server: &LspServer, text: String) {
    let params = lspower::lsp::DidOpenTextDocumentParams {
        text_document: lspower::lsp::TextDocumentItem::new(
            lspower::lsp::Url::parse("file:///home/user/file.flux")
                .unwrap(),
            "flux".to_string(),
            1,
            text,
        ),
    };
    block_on(server.did_open(params));
}

/// Benchmark the response for a package completion
fn server_completion_package(c: &mut Criterion) {
    let fluxscript = r#"import "sql"

sql."#;
    let server = create_server();
    open_file(&server, fluxscript.to_string());

    let params = lspower::lsp::CompletionParams {
        text_document_position:
            lspower::lsp::TextDocumentPositionParams {
                text_document: lspower::lsp::TextDocumentIdentifier {
                    uri: lspower::lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lspower::lsp::Position {
                    line: 2,
                    character: 3,
                },
            },
        work_done_progress_params:
            lspower::lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        partial_result_params: lspower::lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lspower::lsp::CompletionContext {
            trigger_kind:
                lspower::lsp::CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some(".".to_string()),
        }),
    };

    c.bench_function("server completion", |b| {
        b.iter(|| {
            block_on(black_box(server.completion(params.clone())))
                .unwrap()
                .unwrap();
        })
    });
}

fn server_completion_variable(c: &mut Criterion) {
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
    open_file(&server, fluxscript.to_string());

    let params = lspower::lsp::CompletionParams {
        text_document_position:
            lspower::lsp::TextDocumentPositionParams {
                text_document: lspower::lsp::TextDocumentIdentifier {
                    uri: lspower::lsp::Url::parse(
                        "file:///home/user/file.flux",
                    )
                    .unwrap(),
                },
                position: lspower::lsp::Position {
                    line: 8,
                    character: 1,
                },
            },
        work_done_progress_params:
            lspower::lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        partial_result_params: lspower::lsp::PartialResultParams {
            partial_result_token: None,
        },
        context: Some(lspower::lsp::CompletionContext {
            trigger_kind:
                lspower::lsp::CompletionTriggerKind::INVOKED,
            trigger_character: None,
        }),
    };

    c.bench_function("server completion", |b| {
        b.iter(|| {
            block_on(black_box(server.completion(params.clone())))
                .unwrap();
        })
    });
}

criterion_group!(
    benches,
    server_completion_package,
    server_completion_variable
);
criterion_main!(benches);
