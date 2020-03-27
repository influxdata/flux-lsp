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
use crate::shared::{
    find_ident_from_closest, get_bucket, Function, RequestContext,
};
use crate::stdlib::{
    get_builtin_functions, get_package_functions,
    get_specific_package_functions, get_stdlib, Completable,
};
use crate::visitors::ast;
use crate::visitors::semantic::{
    utils, CompletableFinderVisitor, CompletableObjectFinderVisitor,
    FunctionFinderVisitor, ImportFinderVisitor,
    ObjectFunctionFinderVisitor,
};

use flux::ast::walk::walk_rc;
use flux::ast::walk::Node as AstNode;
use flux::ast::{Expression, PropertyKey};
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

async fn get_stdlib_completions(
    name: String,
    imports: Vec<String>,
    ctx: RequestContext,
    src: &str,
) -> Vec<CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib();

    for c in completes.into_iter() {
        if c.matches(name.clone(), imports.clone()) {
            matches.push(c.completion_item(ctx.clone(), src).await);
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
    src: &str,
) -> Result<Vec<CompletionItem>, String> {
    let completables =
        get_user_completables(uri.clone(), pos.clone(), ctx.clone())?;

    let mut result: Vec<CompletionItem> = vec![];
    for x in completables {
        result.push(x.completion_item(ctx.clone(), src).await)
    }

    Ok(result)
}

async fn find_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let pos = params.position.clone();
    let cv = cache::get(uri.clone())?;
    let name =
        find_ident_from_closest(&cv.contents, &params.position);

    let mut items: Vec<CompletionItem> = vec![];
    let imports = get_imports(uri.clone(), pos.clone(), ctx.clone())?;

    let mut stdlib_matches = get_stdlib_completions(
        name.clone(),
        imports.clone(),
        ctx.clone(),
        &cv.contents,
    )
    .await;
    items.append(&mut stdlib_matches);

    let mut user_matches =
        get_user_matches(uri, pos, ctx, &cv.contents).await?;

    items.append(&mut user_matches);

    Ok(CompletionList {
        is_incomplete: false,
        items,
    })
}

fn vec_to_completion_list(v: &[String]) -> CompletionList {
    let items: Vec<CompletionItem> = v
        .iter()
        .map(|value| CompletionItem {
            deprecated: false,
            commit_characters: None,
            detail: None,
            label: format!("\"{}\"", value.trim_end()),
            additional_text_edits: None,
            filter_text: None,
            insert_text: None,
            documentation: None,
            sort_text: None,
            preselect: None,
            insert_text_format: InsertTextFormat::PlainText,
            text_edit: None,
            kind: Some(CompletionItemKind::Text),
        })
        .collect();

    CompletionList {
        is_incomplete: false,
        items,
    }
}

fn new_param_completion(
    name: String,
    trigger: char,
) -> CompletionItem {
    let insert_text = if trigger == '(' {
        format!("{}: ", name)
    } else {
        format!(" {}: ", name)
    };

    CompletionItem {
        deprecated: false,
        commit_characters: None,
        detail: None,
        label: name,
        additional_text_edits: None,
        filter_text: None,
        insert_text: Some(insert_text),
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: InsertTextFormat::Snippet,
        text_edit: None,
        kind: Some(CompletionItemKind::Field),
    }
}

fn get_user_functions(
    uri: String,
    pos: Position,
    ctx: RequestContext,
) -> Result<Vec<Function>, String> {
    let pkg =
        utils::create_completion_package(uri, pos.clone(), ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = FunctionFinderVisitor::new(pos);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).functions.clone());
    }

    Err("failed to get completables".to_string())
}

fn get_provided_arguments(call: &flux::ast::CallExpr) -> Vec<String> {
    let mut provided = vec![];
    if let Some(Expression::Object(obj)) = call.arguments.first() {
        for prop in obj.properties.clone() {
            match prop.key {
                flux::ast::PropertyKey::Identifier(ident) => {
                    provided.push(ident.name)
                }
                flux::ast::PropertyKey::StringLit(lit) => {
                    provided.push(lit.value)
                }
            };
        }
    }

    provided
}

fn get_object_functions(
    uri: String,
    pos: Position,
    ctx: RequestContext,
    object: String,
) -> Result<Vec<Function>, String> {
    let pkg = utils::create_completion_package(uri, pos, ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ObjectFunctionFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok(state
            .results
            .clone()
            .into_iter()
            .filter(|obj| obj.object == object)
            .map(|obj| obj.function)
            .collect());
    }

    Ok(vec![])
}

fn get_function_params(
    name: String,
    functions: Vec<Function>,
    provided: Vec<String>,
) -> Vec<String> {
    functions.into_iter().filter(|f| f.name == name).fold(
        vec![],
        |mut acc, f| {
            acc.extend(
                f.params
                    .into_iter()
                    .filter(|p| !provided.contains(p)),
            );
            acc
        },
    )
}

async fn find_param_completions(
    trigger: char,
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let position = params.position;

    let source = cache::get(uri.clone())?;
    let pkg = crate::utils::create_file_node_from_text(
        uri.clone(),
        source.contents,
    );
    let walker = Rc::new(AstNode::File(&pkg.files[0]));
    let visitor = ast::CallFinderVisitor::new(position.move_back(1));

    walk_rc(&visitor, walker);

    let state = visitor.state.borrow();
    let node = (*state).node.clone();
    let mut items: Vec<String> = vec![];

    if let Some(node) = node {
        if let AstNode::CallExpr(call) = node.as_ref() {
            let provided = get_provided_arguments(call);

            if let Expression::Identifier(ident) = call.callee.clone()
            {
                items.extend(get_function_params(
                    ident.name.clone(),
                    get_builtin_functions(),
                    provided.clone(),
                ));

                if let Ok(user_functions) = get_user_functions(
                    uri.clone(),
                    position.clone(),
                    ctx.clone(),
                ) {
                    items.extend(get_function_params(
                        ident.name,
                        user_functions,
                        provided.clone(),
                    ));
                }
            }
            if let Expression::Member(me) = call.callee.clone() {
                if let Expression::Identifier(ident) = me.object {
                    let package_functions =
                        get_package_functions(ident.name.clone());

                    let object_functions = get_object_functions(
                        uri, position, ctx, ident.name,
                    )?;

                    let key = match me.property {
                        PropertyKey::Identifier(i) => i.name,
                        PropertyKey::StringLit(l) => l.value,
                    };

                    items.extend(get_function_params(
                        key.clone(),
                        package_functions,
                        provided.clone(),
                    ));

                    items.extend(get_function_params(
                        key,
                        object_functions,
                        provided,
                    ));
                }
            }
        }
    }

    Ok(CompletionList {
        is_incomplete: false,
        items: items
            .into_iter()
            .map(|x| new_param_completion(x, trigger.clone()))
            .collect(),
    })
}

async fn find_arg_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let cv = cache::get(uri)?;
    let name =
        &find_ident_from_closest(&cv.contents, &params.position);
    if name == "bucket" {
        let buckets = ctx.callbacks.get_bucket().await?;
        return Ok(vec_to_completion_list(&buckets));
    } else if name == "measurement" || name == "_measurement" {
        let measurements = ctx
            .callbacks
            .get_measurement(get_bucket(&cv.contents))
            .await?;
        return Ok(vec_to_completion_list(&measurements));
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
    let cv = cache::get(uri.clone())?;
    let name = find_ident_from_closest(&cv.contents, &pos);

    let mut list = vec![];
    get_specific_package_functions(&mut list, name.clone());

    let mut items = vec![];
    let obj_results =
        get_specific_object(name, pos, uri.clone(), ctx.clone())?;

    for completable in obj_results.into_iter() {
        items.push(
            completable
                .completion_item(ctx.clone(), &cv.contents)
                .await,
        );
    }

    for item in list.into_iter() {
        items.push(
            item.completion_item(ctx.clone(), &cv.contents).await,
        );
    }

    return Ok(CompletionList {
        is_incomplete: false,
        items,
    });
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
    trigger: char,
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    if trigger == '.' {
        return find_dot_completions(params, ctx).await;
    } else if (vec![':', '=', '<', '>']).contains(&trigger) {
        return find_arg_completions(params, ctx).await;
    } else if trigger == '(' || trigger == ',' {
        return find_param_completions(trigger, params, ctx).await;
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
