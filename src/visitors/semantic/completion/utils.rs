use crate::protocol::properties::Position;

use flux::ast::SourceLocation;
use flux::semantic::nodes::*;
use flux::semantic::types::MonoType;

pub fn follow_function_pipes(c: &CallExpr) -> &MonoType {
    if let Some(p) = &c.pipe {
        if let Expression::Call(call) = p {
            return follow_function_pipes(&call);
        }
    }

    &c.typ
}

pub fn defined_after(loc: &SourceLocation, pos: Position) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return true;
    }

    false
}
