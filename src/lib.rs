#![cfg_attr(feature = "strict", deny(warnings))]
extern crate clap;

mod convert;
mod server;
mod shared;
mod stdlib;
mod wasm;

mod visitors;

pub use server::LspServer;

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
