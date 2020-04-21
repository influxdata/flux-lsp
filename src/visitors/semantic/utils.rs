use crate::cache;
use crate::protocol::properties::Position;
use crate::shared::ast::is_in_node;
use crate::shared::RequestContext;

use std::convert::TryFrom;

use flux::parser::parse_string;
use flux::semantic::convert::convert_with;
use flux::semantic::fresh::Fresher;
use flux::semantic::nodes::Package;

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
    !is_in_node(position, node.base())
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
    ctx: RequestContext,
) -> Result<Package, String> {
    let cv = cache::get(uri.clone())?;
    let contents = remove_character(cv.contents, pos.clone());
    let mut file = parse_string("", contents.as_str());

    file.body = file
        .body
        .into_iter()
        .filter(|x| valid_node(x, pos.clone()))
        .collect();

    let mut pkg =
        crate::shared::create_ast_package(uri.clone(), ctx)?;
    pkg.files = pkg
        .files
        .into_iter()
        .map(
            |curr| if curr.name == uri { file.clone() } else { curr },
        )
        .collect();

    match analyze(pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("{}", e)),
    }
}

pub fn create_completion_package(
    uri: String,
    pos: Position,
    ctx: RequestContext,
) -> Result<Package, String> {
    create_filtered_package(uri, ctx, |x| valid_node(x, pos.clone()))
}

pub fn create_clean_package(
    uri: String,
    ctx: RequestContext,
) -> Result<Package, String> {
    create_filtered_package(uri, ctx, |x| {
        if let flux::ast::Statement::Bad(_) = x {
            return false;
        }
        true
    })
}

fn create_filtered_package<F>(
    uri: String,
    ctx: RequestContext,
    mut filter: F,
) -> Result<Package, String>
where
    F: FnMut(&flux::ast::Statement) -> bool,
{
    let mut ast_pkg =
        crate::shared::create_ast_package(uri.clone(), ctx)?;

    ast_pkg.files = ast_pkg
        .files
        .into_iter()
        .map(|mut file| {
            if file.name == uri.clone() {
                file.body = file
                    .body
                    .into_iter()
                    .filter(|x| filter(x))
                    .collect();
            }

            file
        })
        .collect();

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
