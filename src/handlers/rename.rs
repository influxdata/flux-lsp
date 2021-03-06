use crate::cache::Cache;
use crate::handlers::references::find_references;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::properties::TextEdit;
use crate::protocol::requests::{
    PolymorphicRequest, RenameParams, Request,
};
use crate::protocol::responses::{Response, WorkspaceEditResult};

use std::collections::HashMap;

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

        let mut workspace_edit = WorkspaceEditResult {
            changes: HashMap::new(),
        };

        if let Some(params) = request.params {
            let uri = params.text_document.uri.as_str();
            let new_name = params.new_name;
            let locations =
                find_references(uri, params.position, cache)?;

            for location in locations.iter() {
                let uri = location.uri.clone();

                if workspace_edit.changes.get(&uri.clone()).is_none()
                {
                    workspace_edit
                        .changes
                        .insert(uri.clone(), vec![]);
                }

                if let Some(edits) =
                    workspace_edit.changes.get_mut(&uri.clone())
                {
                    let text_edit = TextEdit {
                        range: location.range.clone(),
                        new_text: new_name.clone(),
                    };

                    edits.push(text_edit);
                }
            }
        }

        let response =
            Response::new(request.id, Some(workspace_edit));

        let json = response.to_json()?;

        Ok(Some(json))
    }
}
