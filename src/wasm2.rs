use std::str;

use js_sys::{Function, Promise};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

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
pub struct Server {}

#[wasm_bindgen]
impl Server {
    #[wasm_bindgen(constructor)]
    pub fn new(
        _disable_folding: bool,
        _support_multiple_files: bool,
    ) -> Self {
        Server {}
    }

    pub fn process(&mut self, msg: String) -> Promise {
        future_to_promise(async move {
            Ok(JsValue::from(ServerResponse {
                message: Some(
                    str::from_utf8(msg.as_bytes())
                        .unwrap()
                        .to_string(),
                ),
                error: None,
            }))
        })
    }

    pub fn register_buckets_callback(&mut self, _f: Function) {}
    pub fn register_measurements_callback(&mut self, _f: Function) {}
    pub fn register_tag_keys_callback(&mut self, _f: Function) {}
    pub fn register_tag_values_callback(&mut self, _f: Function) {}
}
