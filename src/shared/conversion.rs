use flux::ast::check;
use flux::ast::Package;
use flux::parser::parse_string;
use flux::semantic::walk::Node;

use std::rc::Rc;

use lspower::lsp;

pub fn map_node_to_location(
    uri: lsp::Url,
    node: Rc<Node>,
) -> lsp::Location {
    let start_line = node.loc().start.line - 1;
    let start_col = node.loc().start.column - 1;
    let end_line = node.loc().end.line - 1;
    let end_col = node.loc().end.column - 1;

    lsp::Location {
        uri,
        range: lsp::Range {
            start: lsp::Position {
                line: start_line,
                character: start_col,
            },
            end: lsp::Position {
                line: end_line,
                character: end_col,
            },
        },
    }
}

pub fn create_file_node_from_text(
    uri: lsp::Url,
    text: String,
) -> Package {
    parse_string(uri.as_str(), text.as_str()).into()
}

pub fn flux_position_to_position(
    pos: flux::ast::Position,
) -> lsp::Position {
    lsp::Position {
        line: pos.line - 1,
        character: pos.column - 1,
    }
}

pub fn map_errors_to_diagnostics(
    errors: Vec<check::Error>,
) -> Vec<lsp::Diagnostic> {
    let mut result = vec![];

    for error in errors {
        result.push(map_error_to_diagnostic(error));
    }

    result
}

pub fn location_to_range(
    location: flux::ast::SourceLocation,
) -> lsp::Range {
    lsp::Range {
        start: flux_position_to_position(location.start),
        end: flux_position_to_position(location.end),
    }
}

fn map_error_to_diagnostic(error: check::Error) -> lsp::Diagnostic {
    lsp::Diagnostic {
        severity: Some(lsp::DiagnosticSeverity::Error),
        code: Some(lsp::NumberOrString::Number(1)),
        message: error.message,
        range: location_to_range(error.location),

        code_description: None,
        data: None,
        related_information: None,
        source: None,
        tags: None,
    }
}
