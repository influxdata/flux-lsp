#![allow(dead_code, clippy::panic, clippy::unwrap_used)]
use std::sync::{Arc, Mutex};

use futures::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::server::LspServerBuilder;

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
    messages: Arc<Mutex<Vec<u8>>>,
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
        let mut length = messages.len();
        if messages.len() > buffer.len() {
            length = buffer.len();
        }
        buffer[..length].copy_from_slice(&messages[..length]);

        messages.drain(0..length);
        async_std::task::Poll::Ready(Ok(length))
    }
}

struct Outgoing {
    server: Arc<Lsp>,
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl Outgoing {
    pub fn new(server: Arc<Lsp>) -> Self {
        Outgoing {
            server,
            buffer: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl Outgoing {
    /// Convert raw bytes into a vector of messages.
    fn buffer_to_messages(&self) -> Vec<String> {
        // XXX: rockstar (19 Nov 2021) - There is a glaring bug here where, should
        // the response have 'Content-Length:' in it, e.g. the content being formatted,
        // this will get broken in a way that isn't recoverable. The fix is to make
        // parsing stateful, but that can be done without affecting the public API,
        // and that's what's driving this work on a deadline currently.
        let mut byte_count = 0;
        let mut messages = vec![];
        let mut buffer = self.buffer.lock().unwrap();
        let buffer_string = std::str::from_utf8(&buffer).unwrap();
        let possible_messages =
            buffer_string.split("Content-Length:");
        for possible_message in
            possible_messages.filter(|msg| !msg.is_empty())
        {
            let lines: Vec<&str> =
                possible_message.split("\r\n").collect();
            if lines.len() < 3 {
                // Message is not complete yet.
                continue;
            }
            let body: String = if lines.len() > 3 {
                lines[2..].join("\r\n").trim_start().into()
            } else {
                lines[2].into()
            };
            // If lines[1] is not empty, is that a malformed message?
            // Should a check be made?
            let length = lines[0].trim().parse::<usize>().unwrap();
            if length > body.len() {
                // Message body is not complete
                continue;
            }
            let message = if length < body.len() {
                format!(
                    "Content-Length:{}\r\n\r\n{}",
                    lines[0],
                    body[..length].to_string()
                )
            } else {
                format!("Content-Length:{}", possible_message)
            };
            byte_count += message.len();
            messages.push(message);
        }
        buffer.drain(..byte_count);

        messages
    }
}

impl async_std::io::Write for Outgoing {
    #[inline]
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut async_std::task::Context<'_>,
        buffer: &[u8],
    ) -> async_std::task::Poll<async_std::io::Result<usize>> {
        let mut internal_buffer = self.buffer.lock().unwrap();
        internal_buffer.extend_from_slice(&buffer[..buffer.len()]);

        async_std::task::Poll::Ready(Ok(buffer.len()))
    }

    #[inline]
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut async_std::task::Context<'_>,
    ) -> async_std::task::Poll<async_std::io::Result<()>> {
        let messages = self.buffer_to_messages();
        for message in messages {
            self.server.fire(&message);
        }
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
    incoming: Arc<Mutex<Vec<u8>>>,
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
            let mut buffer = incoming.lock().unwrap();
            buffer.extend_from_slice(msg.as_bytes());
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
            lspower::LspService::new(|_client| {
                LspServerBuilder::default().build()
            });
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

    #[test]
    fn test_buffer_to_messages_complete_message() {
        let message = r#"Content-Length: 84

{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}"#.replace("\n", "\r\n");
        let buffer: Vec<u8> = message.as_bytes().into();

        let outgoing = Outgoing {
            server: Arc::new(Lsp::new()),
            buffer: Arc::new(Mutex::new(buffer)),
        };

        let messages = outgoing.buffer_to_messages();

        assert_eq!(messages, vec![message]);
        assert_eq!(outgoing.buffer.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_buffer_to_messages_partial_message_one_line() {
        let message = r#"Content-Length: 8"#;
        let buffer: Vec<u8> = message.as_bytes().into();

        let outgoing = Outgoing {
            server: Arc::new(Lsp::new()),
            buffer: Arc::new(Mutex::new(buffer)),
        };

        let messages = outgoing.buffer_to_messages();

        assert!(messages.is_empty());
        assert_eq!(
            outgoing.buffer.lock().unwrap().len(),
            message.len()
        );
    }

    #[test]
    fn test_buffer_to_messages_partial_message_two_lines() {
        let message = r#"Content-Length: 84
"#
        .replace("\n", "\r\n");
        let buffer: Vec<u8> = message.as_bytes().into();

        let outgoing = Outgoing {
            server: Arc::new(Lsp::new()),
            buffer: Arc::new(Mutex::new(buffer)),
        };

        let messages = outgoing.buffer_to_messages();

        assert!(messages.is_empty());
        assert_eq!(
            outgoing.buffer.lock().unwrap().len(),
            message.len()
        );
    }

    #[test]
    fn test_buffer_to_messages_partial_message() {
        let message = r#"Content-Length: 84

{"method": "initialize", "params": { "capabilities": {}}, "jso"#
            .replace("\n", "\r\n");
        let buffer: Vec<u8> = message.as_bytes().into();

        let outgoing = Outgoing {
            server: Arc::new(Lsp::new()),
            buffer: Arc::new(Mutex::new(buffer)),
        };

        let messages = outgoing.buffer_to_messages();

        assert!(messages.is_empty());
        assert_eq!(
            outgoing.buffer.lock().unwrap().len(),
            message.len()
        );
    }

    /// LSP messages don't have an end message delimiter, as the Content-Length will
    /// indicate how long the body of the message is. This test is for ensuring we
    /// find the correct message, trim the buffer, but retain the beginning of the
    /// next (incomplete) message.
    #[test]
    fn test_buffer_to_messages_additional_message() {
        let message = r#"Content-Length: 84

{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}Content-Le"#.replace("\n", "\r\n");
        let buffer: Vec<u8> = message.as_bytes().into();

        let outgoing = Outgoing {
            server: Arc::new(Lsp::new()),
            buffer: Arc::new(Mutex::new(buffer)),
        };

        let messages = outgoing.buffer_to_messages();

        assert_eq!(messages.len(), 1);
        assert_eq!(
            outgoing.buffer.lock().unwrap().len(),
            "Content-Le".len()
        );
    }

    #[test]
    fn test_buffer_to_messages_additional_message_split() {
        let message = r#"Content-Length: 84

{"method": "initialize", "params": { "capabilities": {}}, "jsonrpc": "2.0", "id": 1}Content-Length: 1"#.replace("\n", "\r\n");
        let buffer: Vec<u8> = message.as_bytes().into();

        let outgoing = Outgoing {
            server: Arc::new(Lsp::new()),
            buffer: Arc::new(Mutex::new(buffer)),
        };

        let messages = outgoing.buffer_to_messages();

        assert_eq!(messages.len(), 1);
        assert_eq!(
            outgoing.buffer.lock().unwrap().len(),
            "Content-Length: 1".len()
        );
    }

    #[test]
    fn test_buffer_to_messages_additional_errant_control_characters_in_body(
    ) {
        let message = r#"Content-Length: 86

{"method": "initialize", "params": { "capabilities": {
}}, "jsonrpc": "2.0", "id": 1}Content-Length: 1"#
            .replace("\n", "\r\n");
        let buffer: Vec<u8> = message.as_bytes().into();

        let outgoing = Outgoing {
            server: Arc::new(Lsp::new()),
            buffer: Arc::new(Mutex::new(buffer)),
        };

        let messages = outgoing.buffer_to_messages();

        // We assert the contents properly in other tests. The length will suffice
        // for the purposes of this test.
        assert_eq!(messages.len(), 1);
        assert_eq!(
            outgoing.buffer.lock().unwrap().len(),
            "Content-Length: 1".len()
        );
    }
}
