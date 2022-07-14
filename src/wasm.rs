#![allow(clippy::panic)]

#[cfg(feature = "fluxlang")]
mod lang;
mod lsp;
#[cfg(test)]
mod tests;

#[cfg(feature = "fluxlang")]
pub use self::lang::Flux;
pub use self::lsp::Lsp;

use flux::{ast, formatter, parser};
use wasm_bindgen::prelude::*;

/// Initialize logging - this requires the "console_log" feature to function,
/// as this library adds 180k to the wasm binary being shipped.
#[allow(non_snake_case, dead_code, clippy::expect_used)]
#[wasm_bindgen]
pub fn initLog() {
    #[cfg(feature = "console_log")]
    console_log::init_with_level(log::Level::Trace)
        .expect("error initializing log");
}

/// Parse flux into an AST representation. The AST will be generated regardless
/// of valid flux. As such, no error handling is needed.
#[deprecated]
#[allow(dead_code, deprecated)]
#[wasm_bindgen]
pub fn parse(script: &str) -> JsValue {
    let mut parser = parser::Parser::new(script);
    let parsed = parser.parse_file("".to_string());

    match JsValue::from_serde(&parsed) {
        Ok(value) => value,
        Err(err) => {
            log::error!("{}", err);
            JsValue::from(script)
        }
    }
}

/// Format a flux script from AST.
///
/// In the event that the flux is invalid syntax, an Err will be returned,
/// which will translate into a JavaScript exception being thrown.
#[deprecated]
#[allow(dead_code, deprecated)]
#[wasm_bindgen]
pub fn format_from_js_file(
    js_file: JsValue,
) -> Result<String, JsValue> {
    match js_file.into_serde::<ast::File>() {
        Ok(file) => match formatter::convert_to_string(&file) {
            Ok(formatted) => Ok(formatted),
            Err(e) => Err(format!("{}", e).into()),
        },
        Err(e) => Err(format!("{}", e).into()),
    }
}

#[cfg(test)]
#[allow(deprecated, clippy::wildcard_imports, clippy::unwrap_used)]
mod test {
    use wasm_bindgen_test::*;

    use super::*;

    #[wasm_bindgen_test]
    async fn test_lsp() {
        let message = r#"{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}"#;

        let mut server = Lsp::new();
        let _ = server.send(message.into());

        let promise = server.run();
        let _result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .unwrap();
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
}
