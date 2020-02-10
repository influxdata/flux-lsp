use crate::utils;
use crate::Handler;

use std::ops::Add;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Server {
    handler: Handler,
}

#[wasm_bindgen]
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

impl Default for Server {
    fn default() -> Server {
        Server::new(false)
    }
}

#[wasm_bindgen]
impl Server {
    #[wasm_bindgen(constructor)]
    pub fn new(disable_folding: bool) -> Server {
        Server {
            handler: Handler::new(disable_folding),
        }
    }

    pub fn process(&mut self, msg: String) -> ServerResponse {
        let mut lines = msg.lines();

        if lines.clone().count() > 2 {
            // Skip content length and spacer
            lines.next();
            lines.next();

            let mut content = String::new();

            for line in lines {
                content = content.add(line);
            }

            if let Ok(req) = utils::parse_request(content) {
                match self.handler.handle(req) {
                    Ok(response) => {
                        if let Some(response) = response {
                            return ServerResponse {
                                message: Some(utils::wrap_message(
                                    response,
                                )),
                                error: None,
                            };
                        } else {
                            return ServerResponse {
                                message: None,
                                error: None,
                            };
                        }
                    }
                    Err(error) => {
                        return ServerResponse {
                            message: None,
                            error: Some(error),
                        }
                    }
                }
            } else {
                return ServerResponse {
                    message: None,
                    error: Some(
                        "Failed to parse message".to_string(),
                    ),
                };
            }
        }

        ServerResponse {
            message: None,
            error: Some("Failed to process message".to_string()),
        }
    }
}
