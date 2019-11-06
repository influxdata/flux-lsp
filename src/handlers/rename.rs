use crate::handlers::references::find_references;
use crate::handlers::RequestHandler;
use crate::structs::{
    PolymorphicRequest, RenameParams, Request, Response, TextEdit,
    WorkspaceEditResult,
};

use std::collections::HashMap;

#[derive(Default)]
pub struct RenameHandler {}

impl RequestHandler for RenameHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String> {
        let request: Request<RenameParams> =
            Request::from_json(prequest.data.as_str())?;

        let mut workspace_edit = WorkspaceEditResult {
            changes: HashMap::new(),
        };

        if let Some(params) = request.params {
            let uri = params.text_document.uri;
            let new_name = params.new_name;
            let locations =
                find_references(uri.clone(), params.position)?;

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

        if let Ok(json) = response.to_json() {
            Ok(json)
        } else {
            Err("Could not create response json".to_string())
        }
    }
}
