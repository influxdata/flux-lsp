#![allow(clippy::panic)]
// XXX: rockstar (15 Mar 2022) - rustwasm generates a spurious () at the end of
// the generated code. This was fixed in mainline, but has not yet had a release.
// See https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

use std::mem;

use flux::{ast, formatter, parser};
use futures::prelude::*;
use lspower::{LspService, MessageStream};
use tower_service::Service;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::LspServer;

/// Initialize logging - this requires the "console_log" feature to function,
/// as this library adds 180k to the wasm binary being shipped.
#[allow(non_snake_case, dead_code, clippy::expect_used)]
#[wasm_bindgen]
pub fn initLog() {
    #[cfg(feature = "console_log")]
    console_log::init_with_level(log::Level::Trace)
        .expect("error initializing log");
}

// MessageProcessor calls handlers for recieved messages.
struct MessageProcessor {
    handlers: Vec<js_sys::Function>,
    messages: MessageStream,
    running: bool,
}

impl MessageProcessor {
    async fn process(mut self) {
        self.running = true;

        // Watch for any messages generated in the service and send them to the client
        while let Some(msg) = self.messages.next().await {
            match serde_json::to_string(&msg) {
                Ok(msg) => {
                    self.fire(&msg);
                }
                Err(err) => {
                    log::error!(
                        "failed to JSON encode message: {}",
                        err
                    );
                    break;
                }
            }
        }
    }
    fn on_message(&mut self, func: js_sys::Function) {
        self.handlers.push(func);
    }
    /// Fire the message handlers with the server message.
    fn fire(&self, msg: &str) {
        if !self.running {
            panic!("Attempted to fire message handlers while server is not running")
        }
        for handler in self.handlers.iter() {
            // Set the context to `undefined` explicitly, so the error
            // message on `this` usage is clear.
            if let Err(err) =
                handler.call1(&JsValue::UNDEFINED, &msg.into())
            {
                log::error!("{:?}", err);
            }
        }
    }
}

/// Lsp is the core lsp server interface.
#[wasm_bindgen]
pub struct Lsp {
    processor: Option<MessageProcessor>,
    service: LspService,
}

impl Default for Lsp {
    fn default() -> Self {
        console_error_panic_hook::set_once();

        let (service, messages) =
            lspower::LspService::new(|client| {
                LspServer::new(Some(client))
            });
        Lsp {
            processor: Some(MessageProcessor {
                handlers: vec![],
                messages,
                running: false,
            }),
            service,
        }
    }
}

#[wasm_bindgen]
impl Lsp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a message handler to server messages.
    /// All handlers must be attached before server.run is called.
    #[allow(non_snake_case)]
    pub fn onMessage(&mut self, func: js_sys::Function) {
        if let Some(processor) = &mut self.processor {
            processor.on_message(func)
        }
    }

    /// Send a message to the server.
    pub fn send(&mut self, msg: String) -> js_sys::Promise {
        let message: lspower::jsonrpc::Incoming =
            match serde_json::from_str(&msg) {
                Ok(value) => value,
                Err(err) => {
                    return future_to_promise(async move {
                        Err(JsValue::from(format!(
                            "failed to decode message JSON: {}",
                            err
                        )))
                    })
                }
            };
        let future =
            std::panic::AssertUnwindSafe(self.service.call(message));
        future_to_promise(
            async move {
                match future.await {
                    Ok(result) => match result {
                        Some(result_inner) => {
                            match serde_json::to_string(&result_inner)
                            {
                                Ok(msg) => {
                                    // Return message JSON
                                    Ok(JsValue::from(msg))
                                }
                                Err(err) => {
                                    Err(JsValue::from(format!(
                                        "failed to encode message JSON: {}",
                                        err
                                    )))
                                }
                            }
                        }
                        // The call didn't have a response, return undefined.
                        // This is expected as many calls are for notifications that are not
                        // expected to have responses.
                        None => Ok(JsValue::UNDEFINED),
                    },
                    Err(err) => Err(JsValue::from(format!( "failed to handle request: {}", err)))
                }
            }
            .catch_unwind()
            .unwrap_or_else(|err| {
                Err(JsValue::from({
                    err.downcast::<String>()
                        .map(|s| *s)
                        .unwrap_or_else(|err| {
                            err.downcast::<&str>()
                                .ok()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| {
                                    "Unknown panic occurred"
                                        .to_string()
                                })
                        })
                }))
            }),
        )
    }

    /// Run the server.
    ///
    /// Note: this will run for the lifetime of the server. It should not be
    /// `await`ed. However, as it returns a Promise, it's a good idea to attach
    /// handlers for completion and error. If the promise ever resolves, the server
    /// is no longer running, which may serve as a hint that attention is needed.
    pub fn run(&mut self) -> js_sys::Promise {
        let processor = match mem::take(&mut self.processor) {
            Some(h) => h,
            None => {
                return future_to_promise(async {
                    Err(JsValue::from_str(
                        "run must not be called twice",
                    ))
                });
            }
        };
        let future = std::panic::AssertUnwindSafe(async move {
            processor.process().await
        });
        future_to_promise(
            async move {
                future.await;
                Ok(JsValue::UNDEFINED)
            }
            .catch_unwind()
            .unwrap_or_else(|err| {
                Err(JsValue::from({
                    err.downcast::<String>()
                        .map(|s| *s)
                        .unwrap_or_else(|err| {
                            err.downcast::<&str>()
                                .ok()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| {
                                    "Unknown panic occurred"
                                        .to_string()
                                })
                        })
                }))
            }),
        )
    }
}

/// Parse flux into an AST representation. The AST will be generated regardless
/// of valid flux. As such, no error handling is needed.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(clippy::wildcard_imports, clippy::unwrap_used)]
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
    |> filter(fn: (r) => r["_measurement"] == "activity")"#;

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
