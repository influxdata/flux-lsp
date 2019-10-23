use crate::structs;
use std::fs;
use url::Url;

pub fn get_content_size(s: String) -> Result<usize, String> {
    let tmp = String::from(s.trim_end());
    let stmp: Vec<&str> = tmp.split(": ").collect();

    match String::from(stmp[1]).parse::<usize>() {
        Ok(size) => return Ok(size),
        Err(_) => return Err("Failed to parse content size".to_string()),
    }
}

pub fn parse_request(content: String) -> Result<structs::PolymorphicRequest, String> {
    let request = structs::BaseRequest::from_json(content.as_str())?;

    let result = structs::PolymorphicRequest {
        base_request: request,
        data: content.clone(),
    };

    return Ok(result);
}

pub fn get_file_contents_from_uri(uri: String) -> Result<String, String> {
    let file_path = match Url::parse(uri.as_str()) {
        Ok(s) => s,
        Err(e) => return Err(format!("Failed to get file path: {}", e)),
    };

    let contents = match fs::read_to_string(file_path.path()) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to read file: {}", e)),
    };

    return Ok(contents);
}
