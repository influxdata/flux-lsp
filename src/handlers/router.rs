use crate::cache::Cache;
use crate::handlers::completion::CompletionHandler;
use crate::handlers::completion_resolve::CompletionResolveHandler;
use crate::handlers::document_change::DocumentChangeHandler;
use crate::handlers::document_close::DocumentCloseHandler;
use crate::handlers::document_formatting::DocumentFormattingHandler;
use crate::handlers::document_open::DocumentOpenHandler;
use crate::handlers::document_save::DocumentSaveHandler;
use crate::handlers::document_symbol::DocumentSymbolHandler;
use crate::handlers::folding::FoldingHandler;
use crate::handlers::goto_definition::GotoDefinitionHandler;
use crate::handlers::hover::HoverHandler;
use crate::handlers::initialize::InitializeHandler;
use crate::handlers::references::FindReferencesHandler;
use crate::handlers::rename::RenameHandler;
use crate::handlers::shutdown::ShutdownHandler;
use crate::handlers::signature_help::SignatureHelpHandler;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::requests::PolymorphicRequest;
use crate::shared::RequestContext;

use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use async_trait::async_trait;

#[wasm_bindgen]
pub struct Router {
    mapping: HashMap<String, Box<dyn RequestHandler>>,
    default_handler: Box<dyn RequestHandler>,
    cache: Cache,
}

#[derive(Default)]
struct NoOpHandler {}

#[async_trait]
impl RequestHandler for NoOpHandler {
    async fn handle(
        &self,
        _: PolymorphicRequest,
        _: RequestContext,
        _: &Cache,
    ) -> Result<Option<String>, Error> {
        Ok(None)
    }
}

impl Router {
    pub fn new(disable_folding: bool) -> Router {
        let mut mapping: HashMap<String, Box<dyn RequestHandler>> =
            HashMap::new();

        mapping.insert(
            "textDocument/references".to_string(),
            Box::new(FindReferencesHandler::default()),
        );
        mapping.insert(
            "textDocument/didChange".to_string(),
            Box::new(DocumentChangeHandler::default()),
        );
        mapping.insert(
            "textDocument/didSave".to_string(),
            Box::new(DocumentSaveHandler::default()),
        );
        mapping.insert(
            "textDocument/didClose".to_string(),
            Box::new(DocumentCloseHandler::default()),
        );
        mapping.insert(
            "textDocument/didOpen".to_string(),
            Box::new(DocumentOpenHandler::default()),
        );
        mapping.insert(
            "textDocument/definition".to_string(),
            Box::new(GotoDefinitionHandler::default()),
        );
        mapping.insert(
            "textDocument/rename".to_string(),
            Box::new(RenameHandler::default()),
        );
        mapping.insert(
            "textDocument/hover".to_string(),
            Box::new(HoverHandler::default()),
        );
        mapping.insert(
            "initialize".to_string(),
            Box::new(InitializeHandler::new(disable_folding)),
        );
        mapping.insert(
            "shutdown".to_string(),
            Box::new(ShutdownHandler::default()),
        );
        mapping.insert(
            "textDocument/foldingRange".to_string(),
            Box::new(FoldingHandler::default()),
        );
        mapping.insert(
            "textDocument/documentSymbol".to_string(),
            Box::new(DocumentSymbolHandler::default()),
        );
        mapping.insert(
            "textDocument/formatting".to_string(),
            Box::new(DocumentFormattingHandler::default()),
        );
        mapping.insert(
            "textDocument/completion".to_string(),
            Box::new(CompletionHandler::default()),
        );
        mapping.insert(
            "completionItem/resolve".to_string(),
            Box::new(CompletionResolveHandler::default()),
        );
        mapping.insert(
            "textDocument/signatureHelp".to_string(),
            Box::new(SignatureHelpHandler::default()),
        );

        Router {
            mapping,
            default_handler: Box::new(NoOpHandler::default()),
            cache: Cache::default(),
        }
    }

    pub async fn route(
        &mut self,
        request: PolymorphicRequest,
        ctx: RequestContext,
    ) -> Result<Option<String>, Error> {
        let method = request.method();
        let handler = match self.mapping.get(&method) {
            Some(h) => h,
            None => &self.default_handler,
        };

        let resp = handler.handle(request, ctx, &self.cache).await?;

        Ok(resp)
    }
}
