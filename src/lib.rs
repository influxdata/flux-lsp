#![cfg_attr(feature = "strict", deny(warnings))]

extern crate clap;

pub mod cache;
pub mod handlers;
pub mod protocol;
pub mod shared;
pub mod stdlib;
pub mod wasm;

#[macro_use]
mod macros;
mod visitors;

pub use handlers::Router;
pub use wasm::Server;

mod server;
pub use server::LspServer;
