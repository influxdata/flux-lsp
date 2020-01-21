use crate::cache;
use crate::protocol::properties::{Location, Position, Range};
use crate::utils::is_in_node;

use std::rc::Rc;

use flux::parser::parse_string;
use flux::semantic::convert::convert_with;
use flux::semantic::fresh::Fresher;
use flux::semantic::nodes::{CallExpr, Expression, Package};
use flux::semantic::types::MonoType;
use flux::semantic::walk::Node;

use libstd::analyze;

fn local_analyze(
    pkg: flux::ast::Package,
) -> Result<flux::semantic::nodes::Package, String> {
    convert_with(pkg, &mut Fresher::default())
}

pub fn analyze_source(
    source: &str,
) -> Result<flux::semantic::nodes::Package, String> {
    let file = parse_string("", source);

    let ast_pkg = flux::ast::Package {
        base: file.base.clone(),
        path: "".to_string(),
        package: "main".to_string(),
        files: vec![file],
    };

    local_analyze(ast_pkg)
}

pub fn create_completion_package(
    uri: String,
    pos: Position,
) -> Result<Package, String> {
    let cv = cache::get(uri)?;
    let mut file = parse_string("", cv.contents.as_str());

    file.body = file
        .body
        .into_iter()
        .filter(|x| !is_in_node(pos.clone(), x.base()))
        .collect();

    let ast_pkg = flux::ast::Package {
        base: file.base.clone(),
        path: "".to_string(),
        package: "main".to_string(),
        files: vec![file],
    };

    match analyze(ast_pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("{}", e)),
    }
}

pub fn create_semantic_package(
    uri: String,
) -> Result<Package, String> {
    let cv = cache::get(uri)?;
    let pkg = analyze_source(cv.contents.as_str())?;

    Ok(pkg)
}

pub fn map_node_to_location(uri: String, node: Rc<Node>) -> Location {
    let start_line = node.loc().start.line - 1;
    let start_col = node.loc().start.column - 1;
    let end_line = node.loc().end.line - 1;
    let end_col = node.loc().end.column - 1;

    Location {
        uri,
        range: Range {
            start: Position {
                line: start_line,
                character: start_col,
            },
            end: Position {
                line: end_line,
                character: end_col,
            },
        },
    }
}

pub fn follow_function_pipes(c: &CallExpr) -> &MonoType {
    if let Some(p) = &c.pipe {
        if let Expression::Call(call) = p {
            return follow_function_pipes(&call);
        }
    }

    &c.typ
}
