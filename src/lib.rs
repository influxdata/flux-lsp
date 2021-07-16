#![cfg_attr(feature = "strict", deny(warnings))]
pub mod cache;
pub mod handlers;
pub mod protocol;
pub mod shared;
pub mod stdlib;
pub mod wasm;

mod visitors;

pub use handlers::Router;
pub use wasm::Server;

#[macro_export]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
