#![cfg_attr(feature = "strict", deny(warnings))]
mod convert;
mod server;
mod shared;
mod stdlib;
mod visitors;
mod wasm;

pub use server::LspServer;

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
