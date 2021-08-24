use flux::ast::SourceLocation;

use lsp_types as lsp;

pub fn defined_after(
    loc: &SourceLocation,
    pos: lsp::Position,
) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return true;
    }

    false
}
