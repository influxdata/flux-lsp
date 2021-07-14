use crate::cache::Cache;
use crate::handlers::references::find_references;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::requests::{
    PolymorphicRequest, RenameParams, Request,
};
use crate::protocol::responses::Response;

use std::collections::HashMap;

use lspower::lsp;

#[derive(Default)]
pub struct RenameHandler {}

#[async_trait::async_trait]
impl RequestHandler for RenameHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        _: crate::shared::RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        let request: Request<RenameParams> =
            Request::from_json(prequest.data.as_str())?;

        let mut changes = HashMap::new();
        if let Some(params) = request.params {
            let document_uri = lsp::Url::parse(params.text_document.uri.as_str()).unwrap();
            let new_name = params.new_name;
            let locations =
                find_references(document_uri, params.position, cache)?;

            for location in locations.iter() {
                if changes.get(&location.uri.clone()).is_none()
                {

                        changes
                        .insert(location.uri.clone(), vec![]);
                }

                if let Some(edits) =
                    changes.get_mut(&location.uri.clone())
                {
                    let text_edit = lsp::TextEdit {
                        range: location.range.clone(),
                        new_text: new_name.clone(),
                    };

                    edits.push(text_edit);
                }
            }
        }

        let response =
            Response::new(request.id, Some(lsp::WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }));

        let json = response.to_json()?;

        Ok(Some(json))
    }
}
