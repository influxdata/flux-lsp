use lsp_types as lsp;

pub fn is_in_node(
    pos: lsp::Position,
    base: &flux::ast::BaseNode,
) -> bool {
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
