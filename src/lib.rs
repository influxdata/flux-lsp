#![cfg_attr(feature = "strict", deny(warnings))]
#![deny(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::wildcard_imports
)]
mod completion;
mod server;
mod shared;
mod stdlib;
mod visitors;
mod wasm;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

pub use server::LspServer;
