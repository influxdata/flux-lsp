#[cfg(not(feature = "lsp2"))]
use crate::shared::callbacks;

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
}

#[cfg(not(feature = "lsp2"))]
#[derive(Clone)]
pub struct RequestContext {
    pub support_multiple_files: bool,
    pub callbacks: callbacks::Callbacks,
}

#[cfg(not(feature = "lsp2"))]
impl RequestContext {
    pub fn new(
        callbacks: callbacks::Callbacks,
        support_multiple_files: bool,
    ) -> Self {
        RequestContext {
            support_multiple_files,
            callbacks,
        }
    }
}
