#![cfg_attr(
    feature = "strict",
    deny(warnings, clippy::print_stdout, clippy::print_stderr)
)]
#![warn(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::wildcard_imports
)]
mod completion;
mod diagnostics;
mod lsp;
mod server;
mod shared;
mod stdlib;
mod visitors;
#[cfg(feature = "wasm")]
mod wasm;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub use server::{Config, LspServer};
