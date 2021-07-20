#![cfg_attr(feature = "strict", deny(warnings))]
extern crate clap;

mod cache;
mod handlers;
mod protocol;
mod server;
mod shared;
mod stdlib;
mod wasm;

mod visitors;

pub use server::LspServer;
