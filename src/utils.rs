use std::fs;

use crate::protocol::properties::{Diagnostic, Position, Range};
use crate::protocol::requests::{BaseRequest, PolymorphicRequest};

use flux::ast::{self, check};
use flux::parser::parse_string;
use url::Url;

pub fn get_content_size(s: String) -> Result<usize, String> {
    let tmp = String::from(s.trim_end());
    let stmp: Vec<&str> = tmp.split(": ").collect();

    match String::from(stmp[1]).parse::<usize>() {
        Ok(size) => Ok(size),
        Err(_) => Err("Failed to parse content size".to_string()),
    }
}

pub fn parse_request(
    content: String,
) -> Result<PolymorphicRequest, String> {
    let request = BaseRequest::from_json(content.as_str())?;

    let result = PolymorphicRequest {
        base_request: request,
        data: content.clone(),
    };

    Ok(result)
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

pub fn create_file_node(uri: String) -> Result<ast::File, String> {
    let file = parse_string(
        uri.as_str(),
        &get_file_contents_from_uri(uri.clone())?,
    );

    Ok(file)
}

pub fn get_file_contents_from_uri(
    uri: String,
) -> Result<String, String> {
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

    Ok(contents)
}

// TODO: figure out if all clients are zero based or if its
//       just vim-lsp if not remove the hard coded
//       subtraction in favor of runtime options
fn map_error_to_diagnostic(error: check::Error) -> Diagnostic {
    Diagnostic {
        severity: 1,
        code: 1,
        message: error.message,
        range: Range {
            start: Position {
                line: error.location.start.line - 1,
                character: error.location.start.column - 1,
            },
            end: Position {
                line: error.location.end.line - 1,
                character: error.location.end.column - 1,
            },
        },
    }
}
