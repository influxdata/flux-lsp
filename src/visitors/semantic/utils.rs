use crate::protocol::properties::{Location, Position, Range};
use crate::utils::get_file_contents_from_uri;
use crate::visitors::semantic::walk::Node;

use std::rc::Rc;

use flux::semantic::analyze_source;
use flux::semantic::nodes::Package;

pub fn create_semantic_package(
    uri: String,
) -> Result<Package, String> {
    let src = &get_file_contents_from_uri(uri.clone())?;
    let pkg = match analyze_source(src) {
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
