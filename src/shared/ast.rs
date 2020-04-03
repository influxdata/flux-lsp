use crate::cache;
use crate::shared::structs::RequestContext;

use crate::protocol::properties::Position;

pub fn is_in_node(pos: Position, base: &flux::ast::BaseNode) -> bool {
    let start_line = base.location.start.line - 1;
    let start_col = base.location.start.column - 1;
    let end_line = base.location.end.line - 1;
    let end_col = base.location.end.column - 1;

    if pos.line < start_line {
        return false;
    }

    if pos.line > end_line {
        return false;
    }

    if pos.line == start_line && pos.character < start_col {
        return false;
    }

    if pos.line == end_line && pos.character > end_col {
        return false;
    }

    true
}

pub fn create_ast_package(
    uri: String,
    ctx: RequestContext,
) -> Result<flux::ast::Package, String> {
    let values =
        cache::get_package(uri.clone(), ctx.support_multiple_files)?;

    let pkgs = values
        .into_iter()
        .map(|v: cache::CacheValue| {
            crate::shared::conversion::create_file_node_from_text(
                v.uri.clone(),
                v.contents,
            )
        })
        .collect::<Vec<flux::ast::Package>>();

    let pkg = pkgs.into_iter().fold(
        None,
        |acc: Option<flux::ast::Package>, pkg| {
            if let Some(mut p) = acc {
                let mut files = pkg.files;
                p.files.append(&mut files);
                return Some(p);
            }

            Some(pkg)
        },
    );

    if let Some(mut pkg) = pkg {
        let mut files = pkg.files;
        files.sort_by(|a, _b| {
            if a.name == uri.clone() {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        });
        pkg.files = files;

        return Ok(pkg);
    }

    Err("Failed to create package".to_string())
}
