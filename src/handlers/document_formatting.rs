use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::properties::{Position, Range, TextEdit};
use crate::protocol::requests::{
    DocumentFormattingParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::Response;

use std::convert::TryFrom;

use flux::formatter;

fn create_range(contents: String) -> Range {
    let lines = contents.split('\n').collect::<Vec<&str>>();
    let last = match lines.last() {
        Some(l) => (*l).to_string(),
        None => String::from(""),
    };
    let line_count: u32 = u32::try_from(lines.len()).unwrap();
    let char_count: u32 = u32::try_from(last.len()).unwrap();

    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: line_count - 1,
            character: char_count,
        },
    }
}

#[derive(Default)]
pub struct DocumentFormattingHandler {}

#[async_trait::async_trait]
impl RequestHandler for DocumentFormattingHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _ctx: crate::shared::RequestContext,
    ) -> Result<Option<String>, String> {
        let request: Request<DocumentFormattingParams> =
            Request::from_json(prequest.data.as_str())?;

        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let cache_value = cache::get(uri)?;
            let file_contents = cache_value.contents;
            let range = create_range(file_contents.clone());

            let formatted = formatter::format(file_contents)?;

            let response: Response<Vec<TextEdit>> = Response::new(
                prequest.base_request.id,
                Some(vec![TextEdit {
                    new_text: formatted,
                    range,
                }]),
            );

            let json = response.to_json()?;

            return Ok(Some(json));
        }

        // Get document contents
        Err("Invalid request".to_string())
    }
}
