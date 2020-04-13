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
use crate::shared::{Function, RequestContext};
use crate::stdlib::{
    get_builtin_functions, get_package_functions,
    get_specific_package_functions, get_stdlib, Completable,
};
use crate::visitors::ast;
use crate::visitors::semantic::{
    utils, CallFinderVisitor, CompletableFinderVisitor,
    CompletableObjectFinderVisitor, FunctionFinderVisitor,
    ImportFinderVisitor, ObjectFunctionFinderVisitor,
};

use flux::ast::walk::walk_rc;
use flux::ast::walk::Node as AstNode;
use flux::ast::{Expression, PropertyKey};
use flux::semantic::nodes::CallExpr;
use flux::semantic::walk;

use async_trait::async_trait;

enum CompletionType {
    Generic,
    Logical,
    Bad,
}

struct CompletionInfo {
    completion_type: CompletionType,
    ident: String,
    bucket: Option<String>,
}

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

fn get_completion_info(
    params: CompletionParams,
) -> Result<Option<CompletionInfo>, String> {
    let uri = params.clone().text_document.uri;
    let position = params.clone().position;

    let source = cache::get(uri.clone())?;
    let pkg = crate::shared::conversion::create_file_node_from_text(
        uri,
        source.contents,
    );
    let walker = Rc::new(AstNode::File(&pkg.files[0]));
    let visitor = ast::NodeFinderVisitor::new(position.move_back(1));

    walk_rc(&visitor, walker);

    let state = visitor.state.borrow();
    let finder_node = (*state).node.clone();

    if let Some(finder_node) = finder_node {
        let bucket = find_bucket(params).unwrap_or(None);

        if let Some(parent) = finder_node.parent {
            if let AstNode::BinaryExpr(be) = parent.node.as_ref() {
                if let Expression::Identifier(left) = be.left.clone()
                {
                    let name = left.name.clone();

                    return Ok(Some(CompletionInfo {
                        completion_type: CompletionType::Logical,
                        ident: name,
                        bucket,
                    }));
                }
            }
        }

        match finder_node.node.as_ref() {
            AstNode::Identifier(ident) => {
                let name = ident.name.clone();
                return Ok(Some(CompletionInfo {
                    completion_type: CompletionType::Generic,
                    ident: name,
                    bucket,
                }));
            }
            AstNode::BadExpr(expr) => {
                let name = expr.text.clone();
                return Ok(Some(CompletionInfo {
                    completion_type: CompletionType::Bad,
                    ident: name,
                    bucket,
                }));
            }
            AstNode::MemberExpr(mbr) => {
                if let Expression::Identifier(ident) = &mbr.object {
                    return Ok(Some(CompletionInfo {
                        completion_type: CompletionType::Generic,
                        ident: ident.name.clone(),
                        bucket,
                    }));
                }
            }
            AstNode::CallExpr(c) => {
                if let Some(arg) = c.arguments.last() {
                    if let Expression::Identifier(ident) = arg {
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Generic,
                            ident: ident.name.clone(),
                            bucket,
                        }));
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

fn follow_pipes_for_bucket(call: Box<CallExpr>) -> Option<String> {
    for arg in call.arguments {
        if arg.key.name == "bucket" {
            if let flux::semantic::nodes::Expression::StringLit(
                value,
            ) = arg.value
            {
                return Some(value.value);
            } else {
                return None;
            }
        }
    }

    if let Some(flux::semantic::nodes::Expression::Call(next)) =
        call.pipe.clone()
    {
        return follow_pipes_for_bucket(next);
    }

    None
}

fn find_bucket(
    params: CompletionParams,
) -> Result<Option<String>, String> {
    let uri = params.text_document.uri;
    let pkg = utils::create_semantic_package(uri)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = CallFinderVisitor::new(params.position);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        if let Some(node) = (*state).node.clone() {
            if let walk::Node::ExprStmt(stmt) = node.as_ref() {
                if let flux::semantic::nodes::Expression::Call(call) =
                    stmt.expression.clone()
                {
                    return Ok(follow_pipes_for_bucket(call));
                }
            }
        }
    }

    Ok(None)
}

async fn find_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.clone().text_document.uri;
    let pos = params.clone().position.clone();
    let info = get_completion_info(params)?;

    let mut items: Vec<CompletionItem> = vec![];
    let imports = get_imports(uri.clone(), pos.clone(), ctx.clone())?;

    if let Some(info) = info {
        match info.completion_type {
            CompletionType::Generic => {
                let mut stdlib_matches = get_stdlib_completions(
                    info.ident.clone(),
                    imports.clone(),
                    ctx.clone(),
                )
                .await;
                items.append(&mut stdlib_matches);

                let mut user_matches =
                    get_user_matches(uri, pos, ctx).await?;

                items.append(&mut user_matches);
            }
            CompletionType::Logical => {
                if info.ident == "measurement" {
                    if let Some(_bucket) = info.bucket {
                        // TODO: Get measurements from callback
                    }
                }
            }
            CompletionType::Bad => {}
        }
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

fn new_param_completion(
    name: String,
    trigger: String,
) -> CompletionItem {
    let insert_text = if trigger == "(" {
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
    trigger: String,
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let position = params.position;

    let source = cache::get(uri.clone())?;
    let pkg = crate::shared::conversion::create_file_node_from_text(
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
    let info = get_completion_info(params.clone())?;

    if let Some(info) = info {
        if info.ident == "bucket" {
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
    let uri = params.clone().text_document.uri;
    let pos = params.clone().position;
    let info = get_completion_info(params.clone())?;

    if let Some(info) = info {
        let mut list = vec![];
        get_specific_package_functions(&mut list, info.ident.clone());

        let mut items = vec![];
        let obj_results = get_specific_object(
            info.ident,
            pos,
            uri.clone(),
            ctx.clone(),
        )?;

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
    } else if trigger == "(" || trigger == "," {
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
