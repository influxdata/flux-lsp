#![allow(dead_code, unused_imports)]
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

/// Parses a string representation of a json file and returns the corresponding JsValue representation
#[wasm_bindgen]
pub fn parse(s: &str) -> JsValue {
    let mut p = Parser::new(s);
    let file = p.parse_file(String::from(""));

    JsValue::from_serde(&file).unwrap()
}

/// Format a JS file.
#[wasm_bindgen]
pub fn format_from_js_file(js_file: JsValue) -> String {
    if let Ok(file) = js_file.into_serde::<File>() {
        if let Ok(converted) = convert_to_string(&file) {
            return converted;
        }
    }
    js_file.as_string().unwrap()
}

