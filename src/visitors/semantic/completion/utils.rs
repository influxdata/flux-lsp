use flux::ast::SourceLocation;
#[cfg(not(feature = "lsp2"))]
use flux::semantic::nodes::*;
#[cfg(not(feature = "lsp2"))]
use flux::semantic::types::MonoType;

use lsp_types as lsp;

#[cfg(not(feature = "lsp2"))]
pub fn follow_function_pipes(c: &CallExpr) -> &MonoType {
    if let Some(Expression::Call(call)) = &c.pipe {
        return follow_function_pipes(call);
    }

    &c.typ
}

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
