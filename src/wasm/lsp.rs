use std::mem;

use futures::prelude::*;
use lspower::{LspService, MessageStream};
use tower_service::Service;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use crate::LspServer;

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
        #[cfg(feature = "console_error_panic_hook")]
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
