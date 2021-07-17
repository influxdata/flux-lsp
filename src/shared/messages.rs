use crate::protocol::{BaseRequest, PolymorphicRequest};

pub fn wrap_message(s: String) -> String {
    let st = s.clone();
    let result = st.as_bytes();
    let size = result.len();

    format!("Content-Length: {}\r\n\r\n{}", size, s)
}

pub fn get_content_size(s: String) -> Result<usize, String> {
    let tmp = String::from(s.trim_end());
    let stmp: Vec<&str> = tmp.split(": ").collect();

    match String::from(stmp[1]).parse::<usize>() {
        Ok(size) => Ok(size),
        Err(_) => Err("Failed to parse content size".to_string()),
    }
}

pub fn create_polymorphic_request(
    content: String,
) -> Result<PolymorphicRequest, String> {
    let request = BaseRequest::from_json(content.as_str())?;

    let result = PolymorphicRequest {
        base_request: request,
        data: content,
    };

    Ok(result)
}
