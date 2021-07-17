use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

fn default_id() -> u32 {
    0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseRequest {
    #[serde(default = "default_id")]
    pub id: u32,
    pub method: String,
}

impl BaseRequest {
    pub fn from_json(s: &str) -> Result<BaseRequest, String> {
        match serde_json::from_str(s) {
            Ok(c) => Ok(c),
            Err(_) => {
                Err("Failed to parse json of BaseRequest".to_string())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PolymorphicRequest {
    pub base_request: BaseRequest,
    pub data: String,
}

impl PolymorphicRequest {
    pub fn method(&self) -> String {
        self.base_request.method.clone()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Request<T> {
    #[serde(default = "default_id")]
    pub id: u32,
    pub method: String,
    pub params: Option<T>,
}

impl<T> Request<T>
where
    T: DeserializeOwned + Clone,
{
    pub fn from_json(s: &str) -> Result<Request<T>, String> {
        match serde_json::from_str(s) {
            Ok(c) => Ok(c),
            Err(e) => {
                Err(format!("Failed to parse json of Request: {}", e))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShutdownParams {}
