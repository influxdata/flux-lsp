use crate::cache;
use crate::protocol::properties::{Location, Position, Range};
use crate::utils::is_in_node;

use std::convert::TryFrom;
use std::rc::Rc;

use flux::parser::parse_string;
use flux::semantic::convert::convert_with;
use flux::semantic::fresh::Fresher;
use flux::semantic::nodes::{CallExpr, Expression, Package};
use flux::semantic::types::MonoType;
use flux::semantic::walk::Node;

use libstd::analyze;

fn local_analyze(
    pkg: flux::ast::Package,
) -> Result<flux::semantic::nodes::Package, String> {
    convert_with(pkg, &mut Fresher::default())
}

pub fn analyze_source(
    source: &str,
) -> Result<flux::semantic::nodes::Package, String> {
    let file = parse_string("", source);

    let ast_pkg = flux::ast::Package {
        base: file.base.clone(),
        path: "".to_string(),
        package: "main".to_string(),
        files: vec![file],
    };

    local_analyze(ast_pkg)
}

fn valid_node(
    node: &flux::ast::Statement,
    position: Position,
) -> bool {
    match node {
        flux::ast::Statement::Bad(_) => false,
        flux::ast::Statement::Expr(expr) => {
            println!(
                "start_line: {} start_col: {}",
                node.base().location.start.line,
                node.base().location.start.column,
            );
            match expr.expression {
                flux::ast::Expression::Identifier(_) => {
                    println!("2");
                    !is_in_node(position, node.base())
                }
                flux::ast::Expression::Bad(_) => {
                    println!("bad expr");
                    false
                }
                _ => true,
            }
        }
        stmt => {
            println!(
                "stmt_type: {} start_line: {} start_col: {}",
                stmt.typ(),
                node.base().location.start.line,
                node.base().location.start.column,
            );
            true
        },
    }
}

fn remove_character(source: String, pos: Position) -> String {
    source
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            if pos.line != u32::try_from(index).unwrap() {
                return line.to_string();
            }

            line.split("").enumerate().fold(
                String::from(""),
                |mut acc, (index, c)| {
                    if pos.character != u32::try_from(index).unwrap()
                    {
                        acc.push_str(c)
                    }

                    acc
                },
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn create_completion_package_removed(
    uri: String,
    pos: Position,
) -> Result<Package, String> {
    let cv = cache::get(uri)?;
    let contents = remove_character(cv.contents, pos.clone());
    let mut file = parse_string("", contents.as_str());

    file.body = file
        .body
        .into_iter()
        .filter(|x| valid_node(x, pos.clone()))
        .collect();

    let ast_pkg = flux::ast::Package {
        base: file.base.clone(),
        path: "".to_string(),
        package: "main".to_string(),
        files: vec![file],
    };

    match analyze(ast_pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("{}", e)),
    }
}

pub fn create_completion_package(
    uri: String,
    pos: Position,
) -> Result<Package, String> {
    let cv = cache::get(uri)?;
    let mut file = parse_string("", cv.contents.as_str());

    // problem: when completion is inside a nested scope, the node that encapsulates
    //  the nested scope is completely removed
    // solutions:
    //   1. capture filtered node and make a package out of it
    //   2. soem nastly munging of the source code...

    file.body = file
        .body
        .into_iter()
        .filter(|x| valid_node(x, pos.clone()))
        .collect();

    let ast_pkg = flux::ast::Package {
        base: file.base.clone(),
        path: "".to_string(),
        package: "main".to_string(),
        files: vec![file],
    };

    match analyze(ast_pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("{}", e)),
    }
}

pub fn create_semantic_package(
    uri: String,
) -> Result<Package, String> {
    let cv = cache::get(uri)?;
    let pkg = analyze_source(cv.contents.as_str())?;

    Ok(pkg)
}

pub fn map_node_to_location(uri: String, node: Rc<Node>) -> Location {
    let start_line = node.loc().start.line - 1;
    let start_col = node.loc().start.column - 1;
    let end_line = node.loc().end.line - 1;
    let end_col = node.loc().end.column - 1;

    Location {
        uri,
        range: Range {
            start: Position {
                line: start_line,
                character: start_col,
            },
            end: Position {
                line: end_line,
                character: end_col,
            },
        },
    }
}

pub fn follow_function_pipes(c: &CallExpr) -> &MonoType {
    if let Some(p) = &c.pipe {
        if let Expression::Call(call) = p {
            return follow_function_pipes(&call);
        }
    }

    &c.typ
}
