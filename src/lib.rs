#[macro_use]
extern crate lazy_static;

pub mod handler;
pub mod handlers;
pub mod protocol;
pub mod shared;
pub mod stdlib;
pub mod utils;
pub mod wasm;

mod cache;
mod visitors;

pub use handler::Handler;
pub use wasm::Server;
