extern crate flux_lsp_lib;

use flux_lsp_lib::protocol::notifications::*;
use flux_lsp_lib::protocol::properties::*;
use flux_lsp_lib::protocol::requests::*;
use flux_lsp_lib::utils;
use flux_lsp_lib::wasm;

fn create_did_open(text: String) -> Request<TextDocumentParams> {
    Request {
        id: 1,
        method: "textDocument/didOpen".to_string(),
        params: Some(TextDocumentParams {
            text_document: TextDocument {
                uri: "some_uri".to_string(),
                language_id: "flux".to_string(),
                version: 1,
                text: text.clone(),
            },
        }),
    }
}

#[test]
fn test_wasm_server() {
    let text =
        std::fs::read_to_string("tests/fixtures/ok.flux").unwrap();
    let request = create_did_open(text);
    let json = serde_json::to_string(&request).unwrap();
    let msg = utils::wrap_message(json);
    let mut server = wasm::Server::default();

    let result = server.process(msg);
    let message = result.get_message().unwrap();

    let notification = create_diagnostics_notification(
        "some_uri".to_string(),
        vec![],
    )
    .unwrap();
    let notification_json = notification.to_json().unwrap();
    let expected = utils::wrap_message(notification_json);

    assert_eq!(expected, message, "expects proper response")
}
