#![cfg_attr(feature = "strict", deny(warnings))]
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

pub use server::LspServer;

#[macro_export]
macro_rules! walk_ast_package {
    ($visitor:expr, $package:ident) => {{
        let mut visitor = $visitor;
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::Package(&$package),
        );
        visitor
    }};
}

#[macro_export]
macro_rules! walk_semantic_package {
    ($visitor:expr, $package:ident) => {{
        let mut visitor_instance = $visitor;
        flux::semantic::walk::walk(
            &mut visitor_instance,
            flux::semantic::walk::Node::Package(&$package),
        );
        visitor_instance
    }};
}
