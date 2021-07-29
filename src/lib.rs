#![cfg_attr(feature = "strict", deny(warnings))]
extern crate clap;

mod cache;
mod convert;
mod handlers;
mod protocol;
#[cfg(feature = "lsp2")]
mod server;
mod shared;
mod stdlib;
#[cfg(not(feature = "lsp2"))]
mod wasm;
#[cfg(feature = "lsp2")]
mod wasm2;

mod visitors;

#[cfg(feature = "lsp2")]
pub use server::LspServer;

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
