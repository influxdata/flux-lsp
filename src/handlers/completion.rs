use std::rc::Rc;
use std::sync::Arc;

use crate::cache;
use crate::handlers::RequestHandler;
use crate::protocol::properties::Position;
use crate::protocol::requests::{
    CompletionParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::{
    CompletionItem, CompletionItemKind, CompletionList,
    InsertTextFormat, Response,
};
use crate::shared::RequestContext;
use crate::stdlib::{
    get_specific_package_functions, get_stdlib, Completable,
};
use crate::visitors::ast;
use crate::visitors::semantic::{
    utils, CompletableFinderVisitor, CompletableObjectFinderVisitor,
    ImportFinderVisitor,
};

use flux::ast::walk::walk_rc;
use flux::semantic::walk;

use async_trait::async_trait;

fn get_imports(
    uri: String,
    pos: Position,
    ctx: RequestContext,
) -> Result<Vec<String>, String> {
    let pkg = utils::create_completion_package(uri, pos, ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
}

fn get_ident_name(
    uri: String,
    position: Position,
) -> Result<Option<String>, String> {
    let source = cache::get(uri.clone())?;
    let pkg = crate::utils::create_file_node_from_text(
        uri,
        source.contents,
    );
    let walker = Rc::new(flux::ast::walk::Node::File(&pkg.files[0]));
    let visitor = ast::NodeFinderVisitor::new(Position {
        line: position.line,
        character: position.character - 1,
    });

    walk_rc(&visitor, walker);

    let state = visitor.state.borrow();
    let node = (*state).node.clone();

    if let Some(node) = node {
        match node.as_ref() {
            flux::ast::walk::Node::Identifier(ident) => {
                let name = ident.name.clone();
                return Ok(Some(name));
            }
            flux::ast::walk::Node::BadExpr(expr) => {
                let name = expr.text.clone();
                return Ok(Some(name));
            }
            flux::ast::walk::Node::MemberExpr(mbr) => {
                if let flux::ast::Expression::Identifier(ident) =
                    &mbr.object
                {
                    return Ok(Some(ident.name.clone()));
                }
            }
            flux::ast::walk::Node::CallExpr(c) => {
                if let Some(arg) = c.arguments.last() {
                    if let flux::ast::Expression::Identifier(ident) =
                        arg
                    {
                        return Ok(Some(ident.name.clone()));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(None)
}

async fn get_stdlib_completions(
    name: String,
    imports: Vec<String>,
    ctx: RequestContext,
) -> Vec<CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib();

    for c in completes.into_iter() {
        if c.matches(name.clone(), imports.clone()) {
            matches.push(c.completion_item(ctx.clone()).await);
        }
    }

    matches
}

fn get_user_completables(
    uri: String,
    pos: Position,
    ctx: RequestContext,
) -> Result<Vec<Arc<dyn Completable + Send + Sync>>, String> {
    let pkg =
        utils::create_completion_package(uri, pos.clone(), ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = CompletableFinderVisitor::new(pos);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).completables.clone());
    }

    Err("failed to get completables".to_string())
}

async fn get_user_matches(
    uri: String,
    pos: Position,
    ctx: RequestContext,
) -> Result<Vec<CompletionItem>, String> {
    let completables =
        get_user_completables(uri.clone(), pos.clone(), ctx.clone())?;

    let mut result: Vec<CompletionItem> = vec![];
    for x in completables {
        result.push(x.completion_item(ctx.clone()).await)
    }

    Ok(result)
}

async fn find_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let pos = params.position.clone();
    let name = get_ident_name(uri.clone(), params.position)?;

    let mut items: Vec<CompletionItem> = vec![];
    let imports = get_imports(uri.clone(), pos.clone(), ctx.clone())?;

    if let Some(name) = name {
        let mut stdlib_matches = get_stdlib_completions(
            name.clone(),
            imports.clone(),
            ctx.clone(),
        )
        .await;
        items.append(&mut stdlib_matches);

        let mut user_matches =
            get_user_matches(uri, pos, ctx).await?;

        items.append(&mut user_matches);
    }

    Ok(CompletionList {
        is_incomplete: false,
        items,
    })
}

fn new_string_arg_completion(value: String) -> CompletionItem {
    CompletionItem {
        deprecated: false,
        commit_characters: None,
        detail: None,
        label: format!("\"{}\"", value),
        additional_text_edits: None,
        filter_text: None,
        insert_text: None,
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: InsertTextFormat::PlainText,
        text_edit: None,
        kind: Some(CompletionItemKind::Text),
    }
}

async fn find_arg_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let name = get_ident_name(uri, params.position)?;

    if let Some(name) = name {
        if name == "bucket" {
            let buckets = ctx.callbacks.get_buckets().await?;

            let items: Vec<CompletionItem> = buckets
                .into_iter()
                .map(new_string_arg_completion)
                .collect();

            return Ok(CompletionList {
                is_incomplete: false,
                items,
            });
        }
    }

    Ok(CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}

async fn find_dot_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let pos = params.position;
    let name = get_ident_name(uri.clone(), pos.clone())?;

    if let Some(name) = name {
        let mut list = vec![];
        get_specific_package_functions(&mut list, name.clone());

        let mut items = vec![];
        let obj_results =
            get_specific_object(name, pos, uri.clone(), ctx.clone())?;

        for completable in obj_results.into_iter() {
            items
                .push(completable.completion_item(ctx.clone()).await);
        }

        for item in list.into_iter() {
            items.push(item.completion_item(ctx.clone()).await);
        }

        return Ok(CompletionList {
            is_incomplete: false,
            items,
        });
    }

    Ok(CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}

pub fn get_specific_object(
    name: String,
    pos: Position,
    uri: String,
    ctx: RequestContext,
) -> Result<Vec<Arc<dyn Completable + Send + Sync>>, String> {
    let pkg =
        utils::create_completion_package_removed(uri, pos, ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = CompletableObjectFinderVisitor::new(name);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok(state.completables.clone());
    }

    Ok(vec![])
}

#[derive(Default)]
pub struct CompletionHandler {}

async fn triggered_completion(
    trigger: String,
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    if trigger == "." {
        return find_dot_completions(params, ctx).await;
    } else if trigger == ":" {
        return find_arg_completions(params, ctx).await;
    }

    find_completions(params, ctx).await
}

#[async_trait]
impl RequestHandler for CompletionHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: RequestContext,
    ) -> Result<Option<String>, String> {
        let req: Request<CompletionParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = req.params {
            if let Some(context) = params.clone().context {
                if let Some(trigger) = context.trigger_character {
                    let completions =
                        triggered_completion(trigger, params, ctx)
                            .await?;

                    let response = Response::new(
                        prequest.base_request.id,
                        Some(completions),
                    );

                    let result = response.to_json()?;

                    return Ok(Some(result));
                }
            }

            let completions = find_completions(params, ctx).await?;

            let response = Response::new(
                prequest.base_request.id,
                Some(completions),
            );

            let result = response.to_json()?;

            return Ok(Some(result));
        }

        Err("invalid completion request".to_string())
    }
}
