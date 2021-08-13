#[cfg(not(feature = "lsp2"))]
use flux::ast::check;
#[cfg(not(feature = "lsp2"))]
use flux::ast::Package;
#[cfg(not(feature = "lsp2"))]
use flux::parser::parse_string;

use lsp_types as lsp;

#[cfg(not(feature = "lsp2"))]
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

#[cfg(not(feature = "lsp2"))]
pub fn map_errors_to_diagnostics(
    errors: Vec<check::Error>,
) -> Vec<lsp::Diagnostic> {
    let mut result = vec![];

    for error in errors {
        result.push(map_error_to_diagnostic(error));
    }

    result
}

#[cfg(not(feature = "lsp2"))]
pub fn location_to_range(
    location: flux::ast::SourceLocation,
) -> lsp::Range {
    lsp::Range {
        start: flux_position_to_position(location.start),
        end: flux_position_to_position(location.end),
    }
}

#[cfg(not(feature = "lsp2"))]
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
