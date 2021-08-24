use lsp_types as lsp;

pub fn flux_position_to_position(
    pos: flux::ast::Position,
) -> lsp::Position {
    lsp::Position {
        line: pos.line - 1,
        character: pos.column - 1,
    }
}
