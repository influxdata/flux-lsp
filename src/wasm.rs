// `rustc` incorrectly identifies the `pub` symbols as dead code, as wasm_bindgen
// is the tool that exports the interfaces. In this case, it is not a reliable lint.
//
// The `wasm_bindgen` macro itself expands to use `panic`, which `clippy::panic`
// isn't a big fan of. _This_ code should not call `panic`, but there isn't a
// way to enforce it on the macro-generated code.
#![allow(dead_code, clippy::panic)]
/// Wasm functionality, including wasm exported functions.
///
use std::ops::Add;
use std::str;

use flux::ast::File;
use flux::formatter::convert_to_string;
use js_sys::{Function, Promise};
use serde::{Deserialize, Serialize};
use tower_service::Service;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::LspServer;
use flux::parser::Parser;

fn wrap_message(s: String) -> String {
    let st = s.clone();
    let result = st.as_bytes();
    let size = result.len();

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
    message: Option<String>,
    error: Option<String>,
}

#[wasm_bindgen]
impl ServerResponse {
    pub fn get_message(&self) -> Option<String> {
        self.message.clone()
    }

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
            match serde_json::from_str(&json_contents) {
                Ok(value) => value,
                Err(err) => {
                    return Promise::resolve(
                        &ServerResponse {
                            message: None,
                            error: Some(format!("{}", err)),
                        }
                        .into(),
                    )
                }
            };
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
                            // Outgoing requests from server to client are
                            // not currently implemented. This should never be
                            // reached.
                            Ok(JsValue::from(ServerResponse {
                                message: None,
                                error: Some("Server attempted to send a request to the client.".into()),
                            }))
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
#[wasm_bindgen]
pub fn parse(script: &str) -> JsValue {
    let mut parser = Parser::new(script);
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

    /// Valid flux is parsed, and a JavaScript object is returned.
    #[wasm_bindgen_test]
    fn test_parse() {
        let script = r#"option task = { name: "beetle", every: 1h }
from(bucket: "inbucket")
  |> range(start: -task.every)
  |> filter(fn: (r) => r["_measurement"] == "activity")
  |> filter(fn: (r) => r["target"] == "crumbs")"#;

        let parsed = parse(&script);

        assert!(parsed.is_object());
    }

    /// Invalid flux is still parsed, and a JavaScript object is returned.
    #[wasm_bindgen_test]
    fn test_parse_invalid() {
        let script = r#"this isn't flux"#;

        let parsed = parse(&script);

        assert!(parsed.is_object());
    }

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

    pub fn jsvalue_to_server_response<T: FromWasmAbi<Abi = u32>>(
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
}
