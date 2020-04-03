use crate::protocol::notifications::{
    create_diagnostics_notification, Notification,
    PublishDiagnosticsParams,
};
use crate::shared::conversion::map_errors_to_diagnostics;

pub mod ast;
pub mod callbacks;
pub mod conversion;
pub mod messages;
pub mod signatures;
pub mod structs;

use combinations::Combinations;

pub use ast::create_ast_package;
pub use structs::{Function, RequestContext};

pub fn all_combos<T>(l: Vec<T>) -> Vec<Vec<T>>
where
    T: std::cmp::Ord + Clone,
{
    let mut result = vec![];
    let length = l.len();

    for i in 1..length {
        let c: Vec<Vec<T>> =
            Combinations::new(l.clone(), i).collect();
        result.extend(c);
    }

    result.push(l);

    result
}

pub fn create_diagnoistics(
    uri: String,
    ctx: RequestContext,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let package = create_ast_package(uri.clone(), ctx)?;
    let walker = flux::ast::walk::Node::Package(&package);
    let errors = flux::ast::check::check(walker);
    let diagnostics = map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri, diagnostics) {
        Ok(msg) => Ok(msg),
        Err(e) => Err(format!("Failed to create diagnostic: {}", e)),
    }
}
