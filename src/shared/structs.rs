use crate::shared::callbacks;

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
}

#[derive(Clone)]
pub struct RequestContext {
    pub support_multiple_files: bool,
    pub callbacks: callbacks::Callbacks,
}

impl RequestContext {
    pub fn new(
        callbacks: callbacks::Callbacks,
        support_multiple_files: bool,
    ) -> Self {
        RequestContext {
            callbacks,
            support_multiple_files,
        }
    }
}
