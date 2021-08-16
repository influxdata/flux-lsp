#[cfg(not(feature = "lsp2"))]
use crate::cache::Cache;
#[cfg(not(feature = "lsp2"))]
use crate::protocol::{
    create_diagnostics_notification, Notification,
};
#[cfg(not(feature = "lsp2"))]
use crate::shared::conversion::map_errors_to_diagnostics;
#[cfg(not(feature = "lsp2"))]
use crate::visitors::ast::package_finder::{
    PackageFinderVisitor, PackageInfo,
};
#[cfg(not(feature = "lsp2"))]
use crate::visitors::ast::NodeFinderVisitor;
#[cfg(not(feature = "lsp2"))]
use crate::visitors::semantic::{
    utils, CallFinderVisitor, Import, ImportFinderVisitor,
};

#[cfg(not(feature = "lsp2"))]
use flux::ast::walk::walk_rc;
#[cfg(not(feature = "lsp2"))]
use flux::ast::walk::Node as AstNode;
#[cfg(not(feature = "lsp2"))]
use flux::ast::{Expression, PropertyKey};
#[cfg(not(feature = "lsp2"))]
use flux::semantic::nodes::CallExpr;

#[cfg(not(feature = "lsp2"))]
use std::rc::Rc;

#[cfg(not(feature = "lsp2"))]
use flux::semantic::walk;

#[cfg(not(feature = "lsp2"))]
use lsp_types as lsp;

pub mod ast;
pub mod callbacks;
pub mod conversion;
#[cfg(not(feature = "lsp2"))]
pub mod messages;
pub mod signatures;
pub mod structs;

use combinations::Combinations;

#[cfg(not(feature = "lsp2"))]
pub use ast::create_ast_package;
pub use structs::Function;
#[cfg(not(feature = "lsp2"))]
pub use structs::RequestContext;

#[cfg(not(feature = "lsp2"))]
fn move_back(position: lsp::Position, count: u32) -> lsp::Position {
    lsp::Position {
        line: position.line,
        character: position.character - count,
    }
}

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

#[cfg(not(feature = "lsp2"))]
pub fn create_diagnoistics(
    uri: lsp::Url,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Notification<lsp::PublishDiagnosticsParams>, String> {
    let package = create_ast_package(uri.clone(), ctx, cache)?;
    let walker = flux::ast::walk::Node::Package(&package);
    let errors = flux::ast::check::check(walker);
    let diagnostics = map_errors_to_diagnostics(errors);

    Ok(create_diagnostics_notification(uri, diagnostics))
}

pub fn get_package_name(name: String) -> Option<String> {
    let items = name.split('/');
    items.last().map(|n| n.to_string())
}
#[cfg(not(feature = "lsp2"))]
#[derive(Clone)]
pub enum CompletionType {
    Generic,
    Logical(flux::ast::Operator),
    CallProperty(String),
    ObjectMember(String),
    Import,
    Bad,
}

#[cfg(not(feature = "lsp2"))]
#[derive(Clone)]
pub struct CompletionInfo {
    pub completion_type: CompletionType,
    pub ident: String,
    pub bucket: Option<String>,
    pub position: lsp::Position,
    pub uri: lsp::Url,
    pub imports: Vec<Import>,
    pub package: Option<PackageInfo>,
}

#[cfg(not(feature = "lsp2"))]
impl CompletionInfo {
    pub fn create(
        params: lsp::CompletionParams,
        ctx: RequestContext,
        cache: &Cache,
    ) -> Result<Option<CompletionInfo>, String> {
        let uri =
            params.text_document_position.text_document.uri.clone();
        let position = params.text_document_position.position;

        let source = cache.get(uri.as_str())?;
        let pkg =
            crate::shared::conversion::create_file_node_from_text(
                uri.clone(),
                source.contents,
            );
        let walker = Rc::new(AstNode::File(&pkg.files[0]));
        let visitor = NodeFinderVisitor::new(move_back(position, 1));

        walk_rc(&visitor, walker);

        let package = PackageFinderVisitor::find(
            uri.clone(),
            ctx.clone(),
            cache,
        )?;

        let state = visitor.state.borrow();
        let finder_node = (*state).node.clone();

        if let Some(finder_node) = finder_node {
            let bucket = find_bucket(params, ctx.clone(), cache)
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
                            position,
                            uri: uri.clone(),
                            imports: get_imports_removed(
                                uri, position, ctx, cache,
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
                        position,
                        uri: uri.clone(),
                        imports: get_imports_removed(
                            uri, position, ctx, cache,
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
                                    position,
                                    uri: uri.clone(),
                                    imports: get_imports(uri, position, ctx,cache)?,
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
                            let name = left.name;

                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Logical(
                                        be.operator.clone(),
                                    ),
                                ident: name,
                                bucket,
                                position,
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, ctx, cache,
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
                                    position,
                                    uri: uri.clone(),
                                    imports: get_imports(
                                        uri, position, ctx, cache,
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
                        let name = left.name;

                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Logical(
                                be.operator.clone(),
                            ),
                            ident: name,
                            bucket,
                            position,
                            uri: uri.clone(),
                            imports: get_imports(
                                uri, position, ctx, cache,
                            )?,
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
                        position,
                        uri: uri.clone(),
                        imports: get_imports(
                            uri, position, ctx, cache,
                        )?,
                        package,
                    }));
                }
                AstNode::BadExpr(expr) => {
                    let name = expr.text.clone();
                    return Ok(Some(CompletionInfo {
                        completion_type: CompletionType::Bad,
                        ident: name,
                        bucket,
                        position,
                        uri: uri.clone(),
                        imports: get_imports(
                            uri, position, ctx, cache,
                        )?,
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
                            position,
                            uri: uri.clone(),
                            imports: get_imports(
                                uri, position, ctx, cache,
                            )?,
                            package,
                        }));
                    }
                }
                AstNode::CallExpr(c) => {
                    if let Some(Expression::Identifier(ident)) =
                        c.arguments.last()
                    {
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Generic,
                            ident: ident.name.clone(),
                            bucket,
                            position,
                            uri: uri.clone(),
                            imports: get_imports(
                                uri, position, ctx, cache,
                            )?,
                            package,
                        }));
                    }
                }
                _ => {}
            }
        }

        Ok(None)
    }
}

#[cfg(not(feature = "lsp2"))]
fn find_bucket(
    params: lsp::CompletionParams,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Option<String>, String> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let pkg = utils::create_clean_package(uri, ctx, cache)?;
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

#[cfg(not(feature = "lsp2"))]
pub fn get_imports(
    uri: lsp::Url,
    pos: lsp::Position,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<Import>, String> {
    let pkg = utils::create_completion_package(uri, pos, ctx, cache)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
}

#[cfg(not(feature = "lsp2"))]
pub fn get_imports_removed(
    uri: lsp::Url,
    pos: lsp::Position,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<Import>, String> {
    let pkg = utils::create_completion_package_removed(
        uri, pos, ctx, cache,
    )?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
}

#[cfg(not(feature = "lsp2"))]
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

#[cfg(test)]
mod tests {
    use super::all_combos;

    #[test]
    fn test_all_combos() {
        let array = vec!["1", "2", "3"];
        let result = all_combos(array);

        assert_eq!(result.len(), 7);
    }
}
