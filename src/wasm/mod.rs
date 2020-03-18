use crate::shared::callbacks::Callbacks;
use crate::shared::RequestContext;
use crate::utils;
use crate::Handler;

use std::cell::RefCell;
use std::ops::Add;
use std::rc::Rc;

use js_sys::{Function, Promise};
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

#[wasm_bindgen]
pub struct Server {
    handler: Rc<RefCell<Handler>>,
    callbacks: Callbacks,
    support_multiple_files: bool,
}

#[wasm_bindgen]
#[derive(Deserialize)]
pub struct ServerResponse {
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

#[wasm_bindgen]
impl Server {
    #[wasm_bindgen(constructor)]
    pub fn new(
        disable_folding: bool,
        support_multiple_files: bool,
    ) -> Server {
        Server {
            handler: Rc::new(RefCell::new(Handler::new(
                disable_folding,
            ))),
            callbacks: Callbacks::default(),
            support_multiple_files,
        }
    }

    pub fn register_buckets_callback(&mut self, f: Function) {
        self.callbacks.register_buckets_callback(f);
    }

    pub fn process(&mut self, msg: String) -> Promise {
        let handler = self.handler.clone();
        let callbacks = self.callbacks.clone();
        let support_multiple_files = self.support_multiple_files;

        future_to_promise(async move {
            let lines = msg.lines();

            if lines.clone().count() > 2 {
                // Skip content length and spacer
                let content = lines
                    .skip(2)
                    .fold(String::new(), |c, l| c.add(l));

                if let Ok(req) =
                    utils::create_polymorphic_request(content)
                {
                    let ctx = RequestContext::new(
                        callbacks.clone(),
                        support_multiple_files,
                    );
                    let mut h = handler.borrow_mut();
                    match (*h).handle(req, ctx).await {
                        Ok(response) => {
                            if let Some(response) = response {
                                return Ok(JsValue::from(
                                    ServerResponse {
                                        message: Some(
                                            utils::wrap_message(
                                                response,
                                            ),
                                        ),
                                        error: None,
                                    },
                                ));
                            } else {
                                return Ok(JsValue::from(
                                    ServerResponse {
                                        message: None,
                                        error: None,
                                    },
                                ));
                            }
                        }
                        Err(error) => {
                            return Ok(JsValue::from(
                                ServerResponse {
                                    message: None,
                                    error: Some(error),
                                },
                            ))
                        }
                    }
                } else {
                    return Ok(JsValue::from(ServerResponse {
                        message: None,
                        error: Some(
                            "Failed to parse message".to_string(),
                        ),
                    }));
                }
            }
            Ok(JsValue::from(ServerResponse {
                message: None,
                error: Some("Failed to process message".to_string()),
            }))
        })
    }
}
