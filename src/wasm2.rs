#![allow(dead_code, clippy::panic, clippy::unwrap_used)]
use std::sync::{Arc, Mutex};

use futures::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::LspServer;

/// Initialize logging - this requires the "console_log" feature to function,
/// as this library adds 180k to the wasm binary being shipped.
#[allow(non_snake_case, dead_code)]
#[wasm_bindgen]
pub fn initLog() {
    #[cfg(feature = "console_log")]
    console_log::init_with_level(log::Level::Trace)
        .expect("error initializing log");
}

struct Incoming {
    messages: Arc<Mutex<Vec<String>>>,
}

impl futures::io::AsyncRead for Incoming {
    #[inline]
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _: &mut async_std::task::Context<'_>,
        buffer: &mut [u8],
    ) -> async_std::task::Poll<async_std::io::Result<usize>> {
        let mut messages = self.messages.lock().unwrap();
        if messages.is_empty() {
            return async_std::task::Poll::Pending;
        }
        let mut byte_count = 0;
        for message in messages.iter() {
            log::debug!("writing message for read: {:?}", message);
            let length = message.len();
            buffer[byte_count..length]
                .copy_from_slice(&message.as_bytes()[..length]);
            byte_count += length;
        }
        // Empty out messages
        messages.retain(|_x| false);
        assert!(messages.len() == 0);
        async_std::task::Poll::Ready(Ok(byte_count))
    }
}

struct Outgoing {
    server: Arc<Lsp>,
    messages: Arc<Mutex<Vec<String>>>,
}

impl Outgoing {
    pub fn new(server: Arc<Lsp>) -> Self {
        Outgoing {
            server,
            messages: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl async_std::io::Write for Outgoing {
    #[inline]
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut async_std::task::Context<'_>,
        buf: &[u8],
    ) -> async_std::task::Poll<async_std::io::Result<usize>> {
        let string = std::str::from_utf8(buf).unwrap();
        log::debug!("string: \"{}\"", string);
        let parts = string.split("Content-Length:");
        let mut byte_count = 0;
        for part in parts.filter(|msg| !msg.is_empty()) {
            let message = format!("Content-Length:{}", part);
            byte_count += message.len();
            self.server.fire(&message);
        }

        async_std::task::Poll::Ready(Ok(byte_count))
    }

    #[inline]
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut async_std::task::Context<'_>,
    ) -> async_std::task::Poll<async_std::io::Result<()>> {
        async_std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _: &mut async_std::task::Context<'_>,
    ) -> async_std::task::Poll<async_std::io::Result<()>> {
        async_std::task::Poll::Ready(Ok(()))
    }
}

/// Lsp is the core lsp server interface.
#[wasm_bindgen]
pub struct Lsp {
    message_handlers: Vec<js_sys::Function>,
    incoming: Arc<Mutex<Vec<String>>>,
}

impl Default for Lsp {
    fn default() -> Self {
        console_error_panic_hook::set_once();

        let incoming = Arc::new(Mutex::new(vec![]));

        Lsp {
            message_handlers: vec![],
            incoming,
        }
    }
}

#[wasm_bindgen]
impl Lsp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fire the message handlers with the server message.
    pub(crate) fn fire(&self, msg: &str) {
        for handler in self.message_handlers.iter() {
            // Set the context to `undefined` explicitly, so the error
            // message on `this` usage is clear.
            log::debug!("firing");
            if let Err(err) =
                handler.call1(&JsValue::UNDEFINED, &msg.into())
            {
                log::error!("{:?}", err);
            }
        }
    }

    /// Attach a message handler to server messages.
    #[allow(non_snake_case)]
    pub fn onMessage(&mut self, func: js_sys::Function) {
        self.message_handlers.push(func)
    }

    /// Send a message to the server.
    pub fn send(&mut self, msg: String) -> js_sys::Promise {
        let incoming = self.incoming.clone();
        future_to_promise(async move {
            let mut messages = incoming.lock().unwrap();
            messages.push(msg);
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Run the server.
    ///
    /// Note: this will run for the lifetime of the server. It should not be
    /// `await`ed. However, as it returns a Promise, it's a good idea to attach
    /// handlers for completion and error. If the promise ever resolves, the server
    /// is no longer running, which may serve as a hint that attention is needed.
    pub fn run(self) -> js_sys::Promise {
        let (service, messages) =
            lspower::LspService::new(|_client| LspServer::default());
        let server = lspower::Server::new(
            Incoming {
                messages: self.incoming.clone(),
            },
            Outgoing::new(Arc::new(self)),
        )
        .interleave(messages);
        let future =
            std::panic::AssertUnwindSafe(server.serve(service));
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

#[cfg(test)]
mod test {
    use wasm_bindgen_test::*;

    use super::*;

    #[wasm_bindgen_test]
    async fn test_lsp() {
        let message = r#"Content-Length: 84

{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}"#;

        let mut server = Lsp::new();
        let _ = server.send(message.into());

        let promise = server.run();
        let _result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .unwrap();
    }
}
