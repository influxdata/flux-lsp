use crate::cache;
use crate::protocol::properties::{Location, Position, Range};

use std::rc::Rc;

use flux::semantic::nodes::Package;
use flux::semantic::walk::Node;

use flux::parser::parse_string;
use flux::semantic::analyze;

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

    match analyze(ast_pkg) {
        Ok(p) => Ok(p),
        Err(_) => Err("failed to analyze source".to_string()),
    }
}

pub fn create_semantic_package(
    uri: String,
) -> Result<Package, String> {
    let cv = cache::get(uri.clone())?;
    let pkg = match analyze_source(cv.contents.as_str()) {
        Ok(pkg) => pkg,
        Err(_) => {
            return Err("Failed to create semantic node".to_string())
        }
    };

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
