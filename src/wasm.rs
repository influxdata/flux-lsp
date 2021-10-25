#![allow(dead_code)]
use std::ops::Add;
use std::str;

#[cfg(not(feature = "api_next"))]
use flux::ast::File;
use flux::formatter::convert_to_string;
use flux::parser::parse_string;
use js_sys::{Function, Promise};
use serde::{Deserialize, Serialize};
use tower_service::Service;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

#[cfg(feature = "api_next")]
use crate::server::parse_and_analyze;
use crate::LspServer;

// In a wasm_bindgen context, the Ok(_) case is the success case,
// and any error results in a `throw`.
#[cfg(feature = "api_next")]
type WasmResult<T> = Result<T, JsValue>;

/// Validate the flux semantically.
///
/// This function is more than just a parse function, because it doesn't
/// validate identifiers, etc.
#[cfg(feature = "api_next")]
#[wasm_bindgen]
pub fn flux_syntax_is_valid(script: &str) -> bool {
    match parse_and_analyze(script) {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(feature = "api_next")]
#[wasm_bindgen]
pub fn format(script: &str) -> WasmResult<String> {
    if flux_syntax_is_valid(script) {
        let ast_file = parse_string("".into(), script);
        match convert_to_string(&ast_file) {
            Ok(formatted) => Ok(formatted),
            Err(e) => Err(format!("{}", e).into()),
        }
    } else {
        Err("Could not format flux. Please check syntax and try again".into())
    }
}

#[cfg(feature = "api_next")]
#[allow(non_snake_case)]
#[wasm_bindgen]
pub struct LSPServerOptions {
    enableFolding: Option<bool>,
    bucketsCallback: Option<Function>,
    measurementsCallback: Option<Function>,
    tagKeysCallback: Option<Function>,
    tagValuesCallback: Option<Function>,
}

#[cfg(feature = "api_next")]
#[wasm_bindgen]
pub struct LSPServer {
    service: lspower::LspService,
}

#[cfg(feature = "api_next")]
#[wasm_bindgen]
impl LSPServer {
    #[wasm_bindgen(constructor)]
    pub fn new(_options: LSPServerOptions) -> Self {
        console_error_panic_hook::set_once();

        let (service, _messages) =
            lspower::LspService::new(|_client| {
                let server = LspServer::default();
                // TODO: hook in options
                server
            });

        LSPServer { service }
    }

    pub fn send(&mut self, msg: String) -> Promise {
        // Assert the header describes the correct length
        let header: String = msg.lines().next().unwrap_or("").into();
        let length = header
            .split(' ')
            .skip(1)
            .collect::<String>()
            .parse::<usize>()
            .unwrap();
        let json_contents: String =
            msg.lines().skip(2).fold(String::new(), |c, l| c.add(l));
        assert!(json_contents.as_bytes().len() == length);

        let message: lspower::jsonrpc::Incoming =
            serde_json::from_str(&json_contents).unwrap();
        let call = self.service.call(message);
        future_to_promise(async move {
            match call.await {
                Ok(result) => match result {
                    Some(result_inner) => match result_inner {
                        lspower::jsonrpc::Outgoing::Response(
                            response,
                        ) => match serde_json::to_string(&response) {
                            Ok(value) => {
                                Ok(wrap_message(value).into())
                            }
                            Err(err) => {
                                Err(format!("{}", err).into())
                            }
                        },
                        lspower::jsonrpc::Outgoing::Request(
                            _client_request,
                        ) => {
                            panic!("Outgoing requests from server to client are not implemented");
                        }
                    },
                    None => {
                        // Some endpoints don't have results,
                        // e.g. textDocument/didOpen
                        Ok(JsValue::NULL)
                    }
                },
                Err(err) => Err(format!("{}", err).into()),
            }
        })
    }
}

fn wrap_message(s: String) -> String {
    let size = s.as_bytes().len();
    format!("Content-Length: {}\r\n\r\n{}", size, s)
}

#[derive(Serialize)]
struct ResponseError {
    code: u32,
    message: String,
}

#[wasm_bindgen]
#[derive(Deserialize)]
struct ServerResponse {
    #[allow(dead_code)]
    message: Option<String>,
    #[allow(dead_code)]
    error: Option<String>,
}

#[wasm_bindgen]
impl ServerResponse {
    #[allow(dead_code)]
    pub fn get_message(&self) -> Option<String> {
        self.message.clone()
    }

    #[allow(dead_code)]
    pub fn get_error(&self) -> Option<String> {
        self.error.clone()
    }
}

#[derive(Serialize)]
struct ServerError {
    id: u32,
    error: ResponseError,
    jsonrpc: String,
}

#[wasm_bindgen]
pub struct Server {
    service: lspower::LspService,
}

#[wasm_bindgen]
impl Server {
    #[wasm_bindgen(constructor)]
    pub fn new(
        disable_folding: bool,
        _support_multiple_files: bool,
    ) -> Self {
        console_error_panic_hook::set_once();

        let (service, _messages) =
            lspower::LspService::new(|_client| {
                let mut server = LspServer::default();
                if disable_folding {
                    server = server.disable_folding();
                }
                server
            });

        Server { service }
    }

    pub fn process(&mut self, msg: String) -> Promise {
        let json_contents: String =
            msg.lines().skip(2).fold(String::new(), |c, l| c.add(l));

        let message: lspower::jsonrpc::Incoming =
            serde_json::from_str(&json_contents).unwrap();
        let call = self.service.call(message);
        future_to_promise(async move {
            match call.await {
                Ok(result) => match result {
                    Some(result_inner) => match result_inner {
                        lspower::jsonrpc::Outgoing::Response(
                            response,
                        ) => match serde_json::to_string(&response) {
                            Ok(value) => {
                                Ok(JsValue::from(ServerResponse {
                                    message: Some(wrap_message(
                                        value,
                                    )),
                                    error: None,
                                }))
                            }
                            Err(err) => {
                                Ok(JsValue::from(ServerResponse {
                                    message: None,
                                    error: Some(format!("{}", err)),
                                }))
                            }
                        },
                        lspower::jsonrpc::Outgoing::Request(
                            _client_request,
                        ) => {
                            panic!("Outgoing requests from server to client are not implemented");
                        }
                    },
                    None => {
                        // Some endpoints don't have results,
                        // e.g. textDocument/didOpen
                        Ok(JsValue::from(ServerResponse {
                            message: None,
                            error: None,
                        }))
                    }
                },
                Err(err) => Ok(JsValue::from(ServerResponse {
                    message: None,
                    error: Some(format!("{}", err)),
                })),
            }
        })
    }

    pub fn register_buckets_callback(&mut self, _f: Function) {}
    pub fn register_measurements_callback(&mut self, _f: Function) {}
    pub fn register_tag_keys_callback(&mut self, _f: Function) {}
    pub fn register_tag_values_callback(&mut self, _f: Function) {}
}

/// Parse flux into an AST representation. The AST will be generated regardless
/// of valid flux. As such, no error handling is needed.
#[cfg(not(feature = "api_next"))]
#[wasm_bindgen]
pub fn parse(script: &str) -> JsValue {
    let parsed = parse_string("".into(), script);
    JsValue::from_serde(&parsed).unwrap()
}

/// Format a flux script from AST.
///
/// In the event that the flux is invalid syntax, an Err will be returned,
/// which will translate into a JavaScript exception being thrown.
#[cfg(not(feature = "api_next"))]
#[wasm_bindgen]
pub fn format_from_js_file(
    js_file: JsValue,
) -> Result<String, JsValue> {
    match js_file.into_serde::<File>() {
        Ok(file) => match convert_to_string(&file) {
            Ok(formatted) => Ok(formatted),
            Err(e) => Err(format!("{}", e).into()),
        },
        Err(e) => Err(format!("{}", e).into()),
    }
}

#[cfg(test)]
mod test {
    #![allow(deprecated)]
    use wasm_bindgen_test::*;

    use super::*;

    /// Valid flux returns true.
    #[cfg(feature = "api_next")]
    #[wasm_bindgen_test]
    fn test_flux_syntax_is_valid() {
        let script = r#"option task = { name: "beetle", every: 1h }
from(bucket: "inbucket")
  |> range(start: -task.every)
  |> filter(fn: (r) => r["_measurement"] == "activity")
  |> filter(fn: (r) => r["target"] == "crumbs")"#;

        assert!(flux_syntax_is_valid(&script))
    }

    /// Invalid flux returns false
    #[cfg(feature = "api_next")]
    #[wasm_bindgen_test]
    fn test_flux_syntax_is_valid_bad_syntax() {
        let script = r#"this isn't good flux"#;

        assert!(!flux_syntax_is_valid(&script))
    }

    /// Valid flux is parsed, and a JavaScript object is returned.
    #[cfg(not(feature = "api_next"))]
    #[wasm_bindgen_test]
    fn test_parse() {
        let script = r#"option task = { name: "beetle", every: 1h }
from(bucket: "inbucket")
  |> range(start: -task.every)
  |> filter(fn: (r) => r["_measurement"] == "activity")
  |> filter(r["target"] == "crumbs")"#;

        let parsed = parse(&script);

        assert!(parsed.is_object());
    }

    #[cfg(feature = "api_next")]
    #[wasm_bindgen_test]
    fn test_format() {
        let expected = r#"option task = {name: "beetle", every: 1h}

from(bucket: "inbucket")
    |> range(start: -task.every)
    |> filter(fn: (r) => r["_measurement"] == "activity")"#;

        let script = r#"option task={name:"beetle",every:1h} from(bucket:"inbucket")
|>range(start:-task.every)|>filter(fn:(r)=>r["_measurement"]=="activity")"#;
        let formatted = format(&script).unwrap();

        assert_eq!(expected, formatted);
    }

    #[cfg(feature = "api_next")]
    #[wasm_bindgen_test]
    fn test_format_invalid() {
        let script = r#"from(bucket:this isn't flux"#;
        let formatted = format(&script);

        assert!(formatted.is_err());
    }

    /// Invalid flux is still parsed, and a JavaScript object is returned.
    #[cfg(not(feature = "api_next"))]
    #[wasm_bindgen_test]
    fn test_parse_invalid() {
        let script = r#"this isn't flux"#;

        let parsed = parse(&script);

        assert!(parsed.is_object());
    }

    #[cfg(not(feature = "api_next"))]
    #[wasm_bindgen_test]
    fn test_format_from_js_file() {
        let expected = r#"option task = {name: "beetle", every: 1h}

from(bucket: "inbucket")
    |> range(start: -task.every)
    |> filter(fn: (r) => r["_measurement"] == "activity")"#;

        let script = r#"option task={name:"beetle",every:1h} from(bucket:"inbucket")
|>range(start:-task.every)|>filter(fn:(r)=>r["_measurement"]=="activity")"#;
        let parsed = parse(&script);

        let formatted = format_from_js_file(parsed).unwrap();

        assert_eq!(expected, formatted);
    }

    #[cfg(not(feature = "api_next"))]
    #[wasm_bindgen_test]
    fn test_format_from_js_file_invalid() {
        let script = r#"from(bucket:this isn't flux"#;
        let parsed = parse(&script);

        if let Err(error) = format_from_js_file(parsed) {
            assert_eq!(
                "invalid type: map, expected a string at line 1 column 2134",
                error.as_string().unwrap()
            );
        } else {
            panic!("Formatting invalid flux did not throw an error");
        }
    }

    // The following code provides helpers for converting a JsValue into the
    // object that it originated from on this side of the wasm boundary.
    // Please see https://github.com/rustwasm/wasm-bindgen/issues/2231 for
    // more information.
    use wasm_bindgen::convert::FromWasmAbi;

    fn jsvalue_to_server_response<T: FromWasmAbi<Abi = u32>>(
        js: JsValue,
    ) -> Result<T, JsValue> {
        let ctor_name = js_sys::Object::get_prototype_of(&js)
            .constructor()
            .name();
        if ctor_name == "ServerResponse" {
            let ptr =
                js_sys::Reflect::get(&js, &JsValue::from_str("ptr"))?;
            let ptr_u32: u32 =
                ptr.as_f64().ok_or(JsValue::NULL)? as u32;
            let foo = unsafe { T::from_abi(ptr_u32) };
            Ok(foo)
        } else {
            Err(JsValue::NULL)
        }
    }

    #[cfg(not(feature = "api_next"))]
    #[wasm_bindgen_test]
    async fn test_server_initialize() {
        let message = r#"Content-Length: 84

{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}"#;
        let mut server = Server::new(true, false);
        let promise = server.process(message.to_string());
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .unwrap();

        assert!(result.is_object());
        let response: ServerResponse =
            jsvalue_to_server_response(result).unwrap();

        let error = response.get_error();
        assert!(error.is_none());

        /* We don't actually care about the _contents_ of the message, just that
         * there is a message. There are other tests that assert the
         * rest of this functionality.
         */
        let message = response.get_message();
        assert!(message.is_some());
    }

    #[cfg(feature = "api_next")]
    #[wasm_bindgen_test]
    async fn test_lspserver_initialize() {
        let expected = "Content-Length: 478\r\n\r\n{\"jsonrpc\":\"2.0\",\"result\":{\"capabilities\":{\"completionProvider\":{\"resolveProvider\":true,\"triggerCharacters\":[\".\",\":\",\"(\",\",\",\"\\\"\"]},\"definitionProvider\":true,\"documentFormattingProvider\":true,\"documentSymbolProvider\":true,\"foldingRangeProvider\":true,\"hoverProvider\":true,\"referencesProvider\":true,\"renameProvider\":true,\"signatureHelpProvider\":{\"retriggerCharacters\":[\"(\"],\"triggerCharacters\":[\"(\"]},\"textDocumentSync\":1},\"serverInfo\":{\"name\":\"flux-lsp\",\"version\":\"2.0\"}},\"id\":1}";

        let message = r#"Content-Length: 84

{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}"#;
        let mut server = LSPServer::new(LSPServerOptions {
            enableFolding: None,
            bucketsCallback: None,
            measurementsCallback: None,
            tagKeysCallback: None,
            tagValuesCallback: None,
        });
        let promise = server.send(message.into());
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .unwrap();

        assert!(result.is_string());
        let response = result.as_string().unwrap();
        assert_eq!(expected, response);
    }
}
