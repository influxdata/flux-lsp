#![allow(clippy::wildcard_imports, clippy::unwrap_used, deprecated)]
use wasm_bindgen_test::*;

use super::*;

#[wasm_bindgen_test]
async fn lsp_run() {
    let message = r#"{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}"#;

    let mut server = Lsp::new();
    let _ = server.send(message.into());

    let promise = server.run();
    let _result =
        wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
}

#[wasm_bindgen_test]
fn flux_new_from_ast() {
    let script = r#"from  ( bucket: "my-bucket"  ) |> range( start: -15m    )"#;
    let flux = Flux::new(&script);

    let ast = flux.ast();
    let flux2 = Flux::from_ast(ast.clone());

    assert_eq!(flux, flux2);
}

#[wasm_bindgen_test]
fn flux_format() {
    let script = r#"from  ( bucket: "my-bucket"  ) |> range( start: -15m    )"#;
    let flux = Flux::new(&script);

    let formatted = flux.format().unwrap();

    let expected = r#"from(bucket: "my-bucket") |> range(start: -15m)
"#;
    assert_eq!(expected, formatted);
}

#[wasm_bindgen_test]
fn flux_is_valid() {
    let script = r#"from  ( bucket: "my-bucket"  ) |> range( start: -15m    )"#;
    let flux = Flux::new(&script);

    assert!(flux.is_valid());
}

#[wasm_bindgen_test]
fn flux_is_valid_bad_type() {
    let script = r#"from(bucket: 1) |> range(start: -15m)"#;
    let flux = Flux::new(&script);

    assert!(!flux.is_valid());
}

#[wasm_bindgen_test]
fn flux_is_valid_invalid_syntax() {
    let script = r#"from(bucket: 1 |> range(start: -15m)"#;
    let flux = Flux::new(&script);

    assert!(!flux.is_valid());
}

/// Valid flux is parsed, and a JavaScript object is returned.
#[wasm_bindgen_test]
fn test_parse() {
    let script = r#"option task = { name: "beetle", every: 1h }
from(bucket: "inbucket")
|> range(start: -task.every)
|> filter(fn: (r) => r["_measurement"] == "activity")
|> filter(fn: (r) => r["target"] == "crumbs")"#;

    let parsed = parse(script);

    assert!(parsed.is_object());
}

/// Invalid flux is still parsed, and a JavaScript object is returned.
#[wasm_bindgen_test]
fn test_parse_invalid() {
    let script = r#"this isn't flux"#;

    let parsed = parse(script);

    assert!(parsed.is_object());
}

#[wasm_bindgen_test]
fn test_format_from_js_file() {
    let expected = r#"option task = {name: "beetle", every: 1h}

from(bucket: "inbucket")
    |> range(start: -task.every)
    |> filter(fn: (r) => r["_measurement"] == "activity")
"#;

    let script = r#"option task={name:"beetle",every:1h} from(bucket:"inbucket")
|>range(start:-task.every)|>filter(fn:(r)=>r["_measurement"]=="activity")"#;
    let parsed = parse(script);

    let formatted = format_from_js_file(parsed).unwrap();

    assert_eq!(expected, formatted);
}

#[wasm_bindgen_test]
fn test_format_from_js_file_invalid() {
    let script = r#"from(bucket:this isn't flux"#;
    let parsed = parse(script);

    if let Err(error) = format_from_js_file(parsed) {
        assert_eq!(
            "invalid type: map, expected a string at line 1 column 2134",
            error.as_string().unwrap()
        );
    } else {
        panic!("Formatting invalid flux did not throw an error");
    }
}
