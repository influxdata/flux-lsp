use std::rc::Rc;
use std::sync::Arc;

use crate::cache::Cache;
use crate::handlers::{Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request, Response};
use crate::shared::{
    get_imports_removed, CompletionInfo, CompletionType, Function,
    RequestContext,
};

use crate::stdlib::{
    get_builtin_functions, get_package_functions, get_package_infos,
    get_specific_package_functions, get_stdlib, Completable,
};
use crate::visitors::ast;
use crate::visitors::semantic::{
    utils, CompletableFinderVisitor, CompletableObjectFinderVisitor,
    FunctionFinderVisitor, ObjectFunctionFinderVisitor,
};

use flux::ast::walk::walk_rc;
use flux::ast::walk::Node as AstNode;
use flux::ast::CallExpr;
use flux::ast::{Expression, PropertyKey};
use flux::semantic::walk;

use async_trait::async_trait;

use lspower::lsp;

fn move_back(position: lsp::Position, count: u32) -> lsp::Position {
    lsp::Position {
        line: position.line,
        character: position.character - count,
    }
}

struct ObjectMember {
    pub object: String,
    pub member: String,
}

impl ObjectMember {
    pub fn from_string(v: String) -> Option<Self> {
        let parts = v.split('.').collect::<Vec<&str>>();
        let object: Option<&&str> = parts.get(0);
        let member: Option<&&str> = parts.get(1);

        if let Some(&object) = object {
            if let Some(&member) = member {
                return Some(ObjectMember {
                    object: object.to_string(),
                    member: member.to_string(),
                });
            }
        }

        None
    }
}

async fn get_stdlib_completions(
    name: String,
    info: CompletionInfo,
    ctx: RequestContext,
) -> Vec<lsp::CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib();

    for c in completes.into_iter() {
        if c.matches(name.clone(), info.clone()) {
            matches.push(
                c.completion_item(ctx.clone(), info.clone()).await,
            );
        }
    }

    matches
}

fn get_user_completables(
    uri: lsp::Url,
    pos: lsp::Position,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<Arc<dyn Completable + Send + Sync>>, Error> {
    let pkg = utils::create_completion_package(uri, pos, ctx, cache)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = CompletableFinderVisitor::new(pos);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).completables.clone());
    }

    Err(Error {
        msg: "failed to get completables".to_string(),
    })
}

async fn get_user_matches(
    info: CompletionInfo,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<lsp::CompletionItem>, Error> {
    let completables = get_user_completables(
        info.uri.clone(),
        info.position,
        ctx.clone(),
        cache,
    )?;

    let mut result: Vec<lsp::CompletionItem> = vec![];
    for x in completables {
        result
            .push(x.completion_item(ctx.clone(), info.clone()).await)
    }

    Ok(result)
}

async fn get_measurement_completions(
    params: lsp::CompletionParams,
    ctx: RequestContext,
    bucket: Option<String>,
) -> Result<Option<lsp::CompletionList>, Error> {
    if let Some(bucket) = bucket {
        let measurements =
            ctx.callbacks.get_measurements(bucket).await?;

        let items: Vec<lsp::CompletionItem> = measurements
            .into_iter()
            .map(|value| {
                new_string_arg_completion(
                    value,
                    get_trigger(params.clone()),
                )
            })
            .collect();

        return Ok(Some(lsp::CompletionList {
            is_incomplete: false,
            items,
        }));
    }

    Ok(None)
}

async fn get_tag_keys_completions(
    ctx: RequestContext,
    bucket: Option<String>,
) -> Result<Option<lsp::CompletionList>, Error> {
    if let Some(bucket) = bucket {
        let tag_keys = ctx.callbacks.get_tag_keys(bucket).await?;

        let items: Vec<lsp::CompletionItem> = tag_keys
            .into_iter()
            .map(|value| lsp::CompletionItem {
                additional_text_edits: None,
                commit_characters: None,
                deprecated: None,
                detail: None,
                documentation: None,
                filter_text: None,
                insert_text: Some(value.clone()),
                label: value,
                insert_text_format: Some(
                    lsp::InsertTextFormat::Snippet,
                ),
                kind: Some(lsp::CompletionItemKind::Property),
                preselect: None,
                sort_text: None,
                text_edit: None,
                command: None,
                data: None,
                insert_text_mode: None,
                tags: None,
            })
            .collect();

        return Ok(Some(lsp::CompletionList {
            is_incomplete: false,
            items,
        }));
    }

    Ok(None)
}

async fn get_tag_values_completions(
    ctx: RequestContext,
    bucket: Option<String>,
    field: Option<String>,
) -> Result<Option<lsp::CompletionList>, Error> {
    if let Some(bucket) = bucket {
        if let Some(field) = field {
            let tag_values =
                ctx.callbacks.get_tag_values(bucket, field).await?;

            let items: Vec<lsp::CompletionItem> = tag_values
                .into_iter()
                .map(|value| lsp::CompletionItem {
                    additional_text_edits: None,
                    commit_characters: None,
                    deprecated: None,
                    detail: None,
                    documentation: None,
                    filter_text: None,
                    insert_text: Some(value.clone()),
                    label: value,
                    insert_text_format: Some(
                        lsp::InsertTextFormat::Snippet,
                    ),
                    kind: Some(lsp::CompletionItemKind::Property),
                    preselect: None,
                    sort_text: None,
                    text_edit: None,
                    command: None,
                    data: None,
                    insert_text_mode: None,
                    tags: None,
                })
                .collect();

            return Ok(Some(lsp::CompletionList {
                is_incomplete: false,
                items,
            }));
        }
    }

    Ok(None)
}

async fn find_completions(
    params: lsp::CompletionParams,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri.clone();
    let info =
        CompletionInfo::create(params.clone(), ctx.clone(), cache)?;

    let mut items: Vec<lsp::CompletionItem> = vec![];

    if let Some(info) = info {
        match info.completion_type {
            CompletionType::Generic => {
                let mut stdlib_matches = get_stdlib_completions(
                    info.ident.clone(),
                    info.clone(),
                    ctx.clone(),
                )
                .await;
                items.append(&mut stdlib_matches);

                let mut user_matches =
                    get_user_matches(info, ctx, cache).await?;

                items.append(&mut user_matches);
            }
            CompletionType::Logical(_operator) => {
                let om =
                    ObjectMember::from_string(info.ident.clone());
                if let Some(om) = om {
                    if om.object == "r" {
                        let list = get_tag_values_completions(
                            ctx,
                            info.bucket,
                            Some(om.member),
                        )
                        .await?;
                        if let Some(list) = list {
                            return Ok(list);
                        }
                    }
                }
            }
            CompletionType::Bad => {}
            CompletionType::CallProperty(_func) => {
                if info.ident == "bucket" {
                    return get_bucket_completions(
                        ctx,
                        get_trigger(params),
                    )
                    .await;
                } else if info.ident == "measurement" {
                    if let Some(list) = get_measurement_completions(
                        params,
                        ctx,
                        info.bucket,
                    )
                    .await?
                    {
                        return Ok(list);
                    }
                } else {
                    return find_param_completions(
                        None, params, ctx, cache,
                    )
                    .await;
                }
            }
            CompletionType::Import => {
                let infos = get_package_infos();

                let imports = get_imports_removed(
                    uri,
                    info.position,
                    ctx,
                    cache,
                )?;

                let mut items = vec![];
                for info in infos {
                    if !(&imports).iter().any(|x| x.path == info.name)
                    {
                        items.push(new_string_arg_completion(
                            info.path,
                            get_trigger(params.clone()),
                        ));
                    }
                }

                return Ok(lsp::CompletionList {
                    is_incomplete: false,
                    items,
                });
            }
            CompletionType::ObjectMember(_obj) => {
                return find_dot_completions(params, ctx, cache)
                    .await;
            }
        }
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items,
    })
}

fn new_string_arg_completion(
    value: String,
    trigger: Option<String>,
) -> lsp::CompletionItem {
    let trigger = trigger.unwrap_or_else(|| "".to_string());
    let insert_text = if trigger == "\"" {
        value
    } else {
        format!("\"{}\"", value)
    };

    lsp::CompletionItem {
        deprecated: None,
        commit_characters: None,
        detail: None,
        label: insert_text.clone(),
        additional_text_edits: None,
        filter_text: None,
        insert_text: Some(insert_text),
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: Some(lsp::InsertTextFormat::Snippet),
        text_edit: None,
        kind: Some(lsp::CompletionItemKind::Value),
        command: None,
        data: None,
        insert_text_mode: None,
        tags: None,
    }
}

fn new_param_completion(
    name: String,
    trigger: Option<String>,
) -> lsp::CompletionItem {
    let insert_text = if let Some(trigger) = trigger {
        if trigger == "(" {
            format!("{}: ", name)
        } else {
            format!(" {}: ", name)
        }
    } else {
        format!("{}: ", name)
    };

    lsp::CompletionItem {
        deprecated: None,
        commit_characters: None,
        detail: None,
        label: name,
        additional_text_edits: None,
        filter_text: None,
        insert_text: Some(insert_text),
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: Some(lsp::InsertTextFormat::Snippet),
        text_edit: None,
        kind: Some(lsp::CompletionItemKind::Field),
        command: None,
        data: None,
        insert_text_mode: None,
        tags: None,
    }
}

fn get_user_functions(
    uri: lsp::Url,
    pos: lsp::Position,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<Function>, Error> {
    let pkg = utils::create_completion_package(uri, pos, ctx, cache)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = FunctionFinderVisitor::new(pos);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).functions.clone());
    }

    Err(Error {
        msg: "failed to get completables".to_string(),
    })
}

fn get_provided_arguments(call: &CallExpr) -> Vec<String> {
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
    uri: lsp::Url,
    pos: lsp::Position,
    ctx: RequestContext,
    object: String,
    cache: &Cache,
) -> Result<Vec<Function>, Error> {
    let pkg = utils::create_completion_package(uri, pos, ctx, cache)?;
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
    trigger: Option<String>,
    params: lsp::CompletionParams,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let source = cache.get(uri.as_str())?;
    let pkg = crate::shared::conversion::create_file_node_from_text(
        uri.clone(),
        source.contents,
    );
    let walker = Rc::new(AstNode::File(&pkg.files[0]));
    let visitor = ast::CallFinderVisitor::new(move_back(position, 1));

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
                    position,
                    ctx.clone(),
                    cache,
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
                        uri, position, ctx, ident.name, cache,
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

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: items
            .into_iter()
            .map(|x| new_param_completion(x, trigger.clone()))
            .collect(),
    })
}

fn get_trigger(params: lsp::CompletionParams) -> Option<String> {
    if let Some(context) = params.context {
        context.trigger_character
    } else {
        None
    }
}

async fn get_bucket_completions(
    ctx: RequestContext,
    trigger: Option<String>,
) -> Result<lsp::CompletionList, Error> {
    let buckets = ctx.callbacks.get_buckets().await?;

    let items: Vec<lsp::CompletionItem> = buckets
        .into_iter()
        .map(|value| {
            new_string_arg_completion(value, trigger.clone())
        })
        .collect();

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items,
    })
}

async fn find_arg_completions(
    params: lsp::CompletionParams,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<lsp::CompletionList, Error> {
    let info =
        CompletionInfo::create(params.clone(), ctx.clone(), cache)?;

    if let Some(info) = info {
        if info.ident == "bucket" {
            return get_bucket_completions(ctx, get_trigger(params))
                .await;
        }
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}

async fn find_dot_completions(
    params: lsp::CompletionParams,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri.clone();
    let pos = params.text_document_position.position;
    let info = CompletionInfo::create(params, ctx.clone(), cache)?;

    if let Some(info) = info.clone() {
        let imports = info.imports.clone();
        if let CompletionType::ObjectMember(om) =
            info.completion_type.clone()
        {
            if om == "r" {
                if let Some(bucket) = info.bucket.clone() {
                    if let Some(list) = get_tag_keys_completions(
                        ctx.clone(),
                        Some(bucket),
                    )
                    .await?
                    {
                        return Ok(list);
                    }
                }
            }
        }

        let mut list = vec![];
        let name = info.ident.clone();
        get_specific_package_functions(
            &mut list,
            name,
            imports.clone(),
        );

        let mut items = vec![];
        let obj_results = get_specific_object(
            info.ident.clone(),
            pos,
            uri,
            ctx.clone(),
            cache,
        )?;

        for completable in obj_results.into_iter() {
            items.push(
                completable
                    .completion_item(ctx.clone(), info.clone())
                    .await,
            );
        }

        for item in list.into_iter() {
            items.push(
                item.completion_item(ctx.clone(), info.clone()).await,
            );
        }

        return Ok(lsp::CompletionList {
            is_incomplete: false,
            items,
        });
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}

pub fn get_specific_object(
    name: String,
    pos: lsp::Position,
    uri: lsp::Url,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<Arc<dyn Completable + Send + Sync>>, Error> {
    let pkg = utils::create_completion_package_removed(
        uri, pos, ctx, cache,
    )?;
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
    trigger: Option<String>,
    params: lsp::CompletionParams,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<lsp::CompletionList, Error> {
    if let Some(ch) = trigger.clone() {
        if ch == "." {
            return find_dot_completions(params, ctx, cache).await;
        } else if ch == ":" {
            return find_arg_completions(params, ctx, cache).await;
        } else if ch == "(" || ch == "," {
            let trgr = trigger;
            return find_param_completions(trgr, params, ctx, cache)
                .await;
        }
    }

    find_completions(params, ctx, cache).await
}

#[async_trait]
impl RequestHandler for CompletionHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        let req: Request<lsp::CompletionParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = req.params {
            if let Some(context) = params.clone().context {
                let completions = triggered_completion(
                    context.trigger_character,
                    params,
                    ctx,
                    cache,
                )
                .await?;

                let response = Response::new(
                    prequest.base_request.id,
                    Some(completions),
                );

                let result = response.to_json()?;

                return Ok(Some(result));
            }

            let completions =
                find_completions(params, ctx, cache).await?;

            let response = Response::new(
                prequest.base_request.id,
                Some(completions),
            );

            let result = response.to_json()?;

            return Ok(Some(result));
        }

        Err(Error {
            msg: "invalid completion request".to_string(),
        })
    }
}
