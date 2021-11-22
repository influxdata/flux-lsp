#![cfg_attr(feature = "strict", deny(warnings))]
#![deny(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::wildcard_imports
)]
mod convert;
mod server;
mod shared;
mod stdlib;
mod visitors;
mod wasm;
#[cfg(feature = "wasm_next")]
mod wasm2;

pub use server::LspServer;
