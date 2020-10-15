use crate::protocol::properties::{
    Diagnostic, Location, Position, Range,
};

use flux::ast::check;
use flux::ast::Package;
use flux::parser::parse_string;
use flux::semantic::walk::Node;

use std::rc::Rc;

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

pub fn create_file_node_from_text(
    uri: &'_ str,
    text: String,
) -> Package {
    parse_string(uri, text.as_str()).into()
}

pub fn flux_position_to_position(
    pos: flux::ast::Position,
) -> Position {
    Position {
        line: pos.line - 1,
        character: pos.column - 1,
    }
}

pub fn map_errors_to_diagnostics(
    errors: Vec<check::Error>,
) -> Vec<Diagnostic> {
    let mut result = vec![];

    for error in errors {
        result.push(map_error_to_diagnostic(error));
    }

    result
}

pub fn location_to_range(
    location: flux::ast::SourceLocation,
) -> Range {
    Range {
        start: flux_position_to_position(location.start),
        end: flux_position_to_position(location.end),
    }
}

fn map_error_to_diagnostic(error: check::Error) -> Diagnostic {
    Diagnostic {
        severity: 1,
        code: 1,
        message: error.message,
        range: location_to_range(error.location),
    }
}
