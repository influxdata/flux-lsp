use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request, Response};

use std::convert::TryFrom;

use flux::formatter;

use lspower::lsp;

fn create_range(contents: String) -> lsp::Range {
    let lines = contents.split('\n').collect::<Vec<&str>>();
    let last = match lines.last() {
        Some(l) => (*l).to_string(),
        None => String::from(""),
    };
    let line_count: u32 = u32::try_from(lines.len()).unwrap();
    let char_count: u32 = u32::try_from(last.len()).unwrap();

    lsp::Range {
        start: lsp::Position {
            line: 0,
            character: 0,
        },
        end: lsp::Position {
            line: line_count - 1,
            character: char_count,
        },
    }
}

#[derive(Default)]
pub struct DocumentFormattingHandler {}

impl From<flux::Error> for Error {
    fn from(e: flux::Error) -> Error {
        Error { msg: e.msg }
    }
}

#[async_trait::async_trait]
impl RequestHandler for DocumentFormattingHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _ctx: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        let request: Request<lsp::DocumentFormattingParams> =
            Request::from_json(prequest.data.as_str())?;

        if let Some(params) = request.params {
            let uri = params.text_document.uri.as_str();
            let cache_value = cache.get(uri)?;
            let file_contents = cache_value.contents;
            let range = create_range(file_contents.clone());

            let formatted =
                formatter::format(file_contents.as_str())?;

            let response: Response<Vec<lsp::TextEdit>> =
                Response::new(
                    prequest.base_request.id,
                    Some(vec![lsp::TextEdit {
                        new_text: formatted,
                        range,
                    }]),
                );

            let json = response.to_json()?;

            return Ok(Some(json));
        }

        // Get document contents
        Err(Error {
            msg: "Invalid request".to_string(),
        })
    }
}
