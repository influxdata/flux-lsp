#![cfg_attr(feature = "strict", deny(warnings))]
mod cache;
mod handlers;
mod protocol;
mod shared;
mod stdlib;
mod wasm;

mod visitors;

#[macro_export]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
