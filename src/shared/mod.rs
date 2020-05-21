use crate::cache;
use crate::protocol::notifications::{
    create_diagnostics_notification, Notification,
    PublishDiagnosticsParams,
};
use crate::protocol::properties::Position;
use crate::protocol::requests::CompletionParams;
use crate::shared::conversion::map_errors_to_diagnostics;
use crate::visitors::ast::package_finder::{
    PackageFinderVisitor, PackageInfo,
};
use crate::visitors::ast::NodeFinderVisitor;
use crate::visitors::semantic::{
    utils, CallFinderVisitor, Import, ImportFinderVisitor,
};

use flux::ast::walk::walk_rc;
use flux::ast::walk::Node as AstNode;
use flux::ast::{Expression, PropertyKey};
use flux::semantic::nodes::CallExpr;

use std::rc::Rc;

use flux::semantic::walk;

pub mod ast;
pub mod callbacks;
pub mod conversion;
pub mod messages;
pub mod signatures;
pub mod structs;

use combinations::Combinations;

pub use ast::create_ast_package;
pub use structs::{Function, RequestContext};

pub fn all_combos<T>(l: Vec<T>) -> Vec<Vec<T>>
where
    T: std::cmp::Ord + Clone,
{
    let mut result = vec![];
    let length = l.len();

    for i in 1..length {
        let c: Vec<Vec<T>> =
            Combinations::new(l.clone(), i).collect();
        result.extend(c);
    }

    result.push(l);

    result
}

pub fn create_diagnoistics(
    uri: String,
    ctx: RequestContext,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let package = create_ast_package(uri.clone(), ctx)?;
    let walker = flux::ast::walk::Node::Package(&package);
    let errors = flux::ast::check::check(walker);
    let diagnostics = map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri, diagnostics) {
        Ok(msg) => Ok(msg),
        Err(e) => Err(format!("Failed to create diagnostic: {}", e)),
    }
}

pub fn get_package_name(name: String) -> Option<String> {
    let items = name.split('/');

    if let Some(n) = items.last() {
        Some(n.to_string())
    } else {
        None
    }
}
#[derive(Clone)]
pub enum CompletionType {
    Generic,
    Logical(flux::ast::Operator),
    CallProperty(String),
    ObjectMember(String),
    Import,
    Bad,
}

#[derive(Clone)]
pub struct CompletionInfo {
    pub completion_type: CompletionType,
    pub ident: String,
    pub bucket: Option<String>,
    pub position: Position,
    pub uri: String,
    pub imports: Vec<Import>,
    pub package: Option<PackageInfo>,
}

impl CompletionInfo {
    pub fn create(
        params: CompletionParams,
        ctx: RequestContext,
    ) -> Result<Option<CompletionInfo>, String> {
        let uri = params.clone().text_document.uri;
        let position = params.clone().position;

        let source = cache::get(uri.clone())?;
        let pkg =
            crate::shared::conversion::create_file_node_from_text(
                uri.clone(),
                source.contents,
            );
        let walker = Rc::new(AstNode::File(&pkg.files[0]));
        let visitor = NodeFinderVisitor::new(position.move_back(1));

        walk_rc(&visitor, walker);

        let package =
            PackageFinderVisitor::find(uri.clone(), ctx.clone())?;

        let state = visitor.state.borrow();
        let finder_node = (*state).node.clone();

        if let Some(finder_node) = finder_node {
            let bucket = find_bucket(params.clone(), ctx.clone())
                .unwrap_or(None);

            if let Some(parent) = finder_node.parent {
                if let AstNode::MemberExpr(me) = parent.node.as_ref()
                {
                    if let Expression::Identifier(obj) =
                        me.object.clone()
                    {
                        return Ok(Some(CompletionInfo {
                            completion_type:
                                CompletionType::ObjectMember(
                                    obj.name.clone(),
                                ),
                            ident: obj.name,
                            bucket,
                            position: position.clone(),
                            uri: uri.clone(),
                            imports: get_imports_removed(
                                uri, position, ctx,
                            )?,
                            package,
                        }));
                    }
                }

                if let AstNode::ImportDeclaration(_id) =
                    parent.node.as_ref()
                {
                    return Ok(Some(CompletionInfo {
                        completion_type: CompletionType::Import,
                        ident: "".to_string(),
                        bucket,
                        position: position.clone(),
                        uri: uri.clone(),
                        imports: get_imports_removed(
                            uri, position, ctx,
                        )?,
                        package,
                    }));
                }

                if let Some(grandparent) = parent.parent {
                    if let Some(greatgrandparent) = grandparent.parent
                    {
                        if let AstNode::Property(prop) =
                            parent.node.as_ref()
                        {
                            if let AstNode::ObjectExpr(_) =
                                grandparent.node.as_ref()
                            {
                                if let AstNode::CallExpr(call) =
                                    greatgrandparent.node.as_ref()
                                {
                                    let name = match prop.key.clone()
                                    {
                                        PropertyKey::Identifier(
                                            ident,
                                        ) => ident.name,
                                        PropertyKey::StringLit(
                                            lit,
                                        ) => lit.value,
                                    };

                                    if let Expression::Identifier(
                                        func,
                                    ) = call.callee.clone()
                                    {
                                        return Ok(Some(CompletionInfo {
                                    completion_type: CompletionType::CallProperty(func.name), ident: name,
                                    bucket,
                                    position: position.clone(),
                                    uri: uri.clone(),
                                    imports: get_imports(uri, position, ctx)?,
                                    package,
                                }));
                                    }
                                }
                            }
                        }
                    }
                }

                if let AstNode::BinaryExpr(be) = parent.node.as_ref()
                {
                    match be.left.clone() {
                        Expression::Identifier(left) => {
                            let name = left.name.clone();

                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Logical(
                                        be.operator.clone(),
                                    ),
                                ident: name,
                                bucket,
                                position: position.clone(),
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, ctx,
                                )?,
                                package,
                            }));
                        }
                        Expression::Member(left) => {
                            if let Expression::Identifier(ident) =
                                left.object
                            {
                                let key = match left.property {
                                    PropertyKey::Identifier(
                                        ident,
                                    ) => ident.name,
                                    PropertyKey::StringLit(lit) => {
                                        lit.value
                                    }
                                };

                                let name =
                                    format!("{}.{}", ident.name, key);

                                return Ok(Some(CompletionInfo {
                                    completion_type:
                                        CompletionType::Logical(
                                            be.operator.clone(),
                                        ),
                                    ident: name,
                                    bucket,
                                    position: position.clone(),
                                    uri: uri.clone(),
                                    imports: get_imports(
                                        uri, position, ctx,
                                    )?,
                                    package,
                                }));
                            }
                        }
                        _ => {}
                    }
                }
            }

            match finder_node.node.as_ref() {
                AstNode::BinaryExpr(be) => {
                    if let Expression::Identifier(left) =
                        be.left.clone()
                    {
                        let name = left.name.clone();

                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Logical(
                                be.operator.clone(),
                            ),
                            ident: name,
                            bucket,
                            position: position.clone(),
                            uri: uri.clone(),
                            imports: get_imports(uri, position, ctx)?,
                            package,
                        }));
                    }
                }
                AstNode::Identifier(ident) => {
                    let name = ident.name.clone();
                    return Ok(Some(CompletionInfo {
                        completion_type: CompletionType::Generic,
                        ident: name,
                        bucket,
                        position: position.clone(),
                        uri: uri.clone(),
                        imports: get_imports(uri, position, ctx)?,
                        package,
                    }));
                }
                AstNode::BadExpr(expr) => {
                    let name = expr.text.clone();
                    return Ok(Some(CompletionInfo {
                        completion_type: CompletionType::Bad,
                        ident: name,
                        bucket,
                        position: position.clone(),
                        uri: uri.clone(),
                        imports: get_imports(uri, position, ctx)?,
                        package,
                    }));
                }
                AstNode::MemberExpr(mbr) => {
                    if let Expression::Identifier(ident) = &mbr.object
                    {
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Generic,
                            ident: ident.name.clone(),
                            bucket,
                            position: position.clone(),
                            uri: uri.clone(),
                            imports: get_imports(uri, position, ctx)?,
                            package,
                        }));
                    }
                }
                AstNode::CallExpr(c) => {
                    if let Some(arg) = c.arguments.last() {
                        if let Expression::Identifier(ident) = arg {
                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Generic,
                                ident: ident.name.clone(),
                                bucket,
                                position: position.clone(),
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, ctx,
                                )?,
                                package,
                            }));
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(None)
    }
}

fn find_bucket(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<Option<String>, String> {
    let uri = params.text_document.uri;
    let pos = params.position;
    let pkg = utils::create_clean_package(uri, ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = CallFinderVisitor::new(pos);

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

pub fn get_imports(
    uri: String,
    pos: Position,
    ctx: RequestContext,
) -> Result<Vec<Import>, String> {
    let pkg = utils::create_completion_package(uri, pos, ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
}

pub fn get_imports_removed(
    uri: String,
    pos: Position,
    ctx: RequestContext,
) -> Result<Vec<Import>, String> {
    let pkg =
        utils::create_completion_package_removed(uri, pos, ctx)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
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
