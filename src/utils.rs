use std::fs;
use std::rc::Rc;

use crate::structs;

use flux::ast::{self, check, walk};
use flux::parser::parse_string;
use url::Url;

pub fn get_content_size(s: String) -> Result<usize, String> {
    let tmp = String::from(s.trim_end());
    let stmp: Vec<&str> = tmp.split(": ").collect();

    match String::from(stmp[1]).parse::<usize>() {
        Ok(size) => return Ok(size),
        Err(_) => {
            return Err("Failed to parse content size".to_string())
        }
    }
}

pub fn parse_request(
    content: String,
) -> Result<structs::PolymorphicRequest, String> {
    let request = structs::BaseRequest::from_json(content.as_str())?;

    let result = structs::PolymorphicRequest {
        base_request: request,
        data: content.clone(),
    };

    return Ok(result);
}

pub fn map_node_to_location(
    uri: String,
    node: Rc<walk::Node>,
) -> structs::Location {
    let start_line = node.base().location.start.line - 1;
    let start_col = node.base().location.start.column - 1;
    let end_line = node.base().location.end.line - 1;
    let end_col = node.base().location.end.column - 1;

    structs::Location {
        uri,
        range: structs::Range {
            start: structs::Position {
                line: start_line,
                character: start_col,
            },
            end: structs::Position {
                line: end_line,
                character: end_col,
            },
        },
    }
}

pub fn map_errors_to_diagnostics(
    errors: Vec<check::Error>,
) -> Vec<structs::Diagnostic> {
    let mut result = vec![];

    for error in errors {
        result.push(map_error_to_diagnostic(error));
    }

    return result;
}

pub fn create_file_node<'a>(
    uri: String,
) -> Result<ast::File, String> {
    let file = parse_string(
        uri.as_str(),
        &get_file_contents_from_uri(uri.clone())?,
    );

    return Ok(file);
}

fn get_file_contents_from_uri(uri: String) -> Result<String, String> {
    let file_path = match Url::parse(uri.as_str()) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("Failed to get file path: {}", e))
        }
    };

    let contents = match fs::read_to_string(file_path.path()) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to read file: {}", e)),
    };

    return Ok(contents);
}

// TODO: figure out if all clients are zero based or if its
//       just vim-lsp if not remove the hard coded
//       subtraction in favor of runtime options
fn map_error_to_diagnostic(
    error: check::Error,
) -> structs::Diagnostic {
    structs::Diagnostic {
        severity: 1,
        code: 1,
        message: error.message,
        range: structs::Range {
            start: structs::Position {
                line: error.location.start.line - 1,
                character: error.location.start.column - 1,
            },
            end: structs::Position {
                line: error.location.end.line - 1,
                character: error.location.end.column - 1,
            },
        },
    }
}
