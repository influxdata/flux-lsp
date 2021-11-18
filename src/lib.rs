#![cfg_attr(feature = "strict", deny(warnings))]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod convert;
mod server;
mod shared;
mod stdlib;
mod visitors;
mod wasm;
#[cfg(feature = "wasm_next")]
mod wasm2;

pub use server::LspServer;
