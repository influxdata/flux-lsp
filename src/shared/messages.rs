use crate::protocol::requests::{BaseRequest, PolymorphicRequest};

pub fn wrap_message(s: String) -> String {
    let st = s.clone();
    let result = st.as_bytes();
    let size = result.len();

    format!("Content-Length: {}\r\n\r\n{}", size, s)
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
