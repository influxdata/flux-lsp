use crate::structs;

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
