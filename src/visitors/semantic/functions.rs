use std::cell::RefCell;
use std::rc::Rc;

use flux::ast::SourceLocation;
use flux::semantic::nodes::Expression;
use flux::semantic::types::MonoType;
use flux::semantic::walk::{Node, Visitor};

use crate::protocol::properties::Position;
use crate::shared::signatures::FunctionInfo;

#[derive(Default)]
pub struct FunctionFinderState {
    pub functions: Vec<Rc<FunctionInfo>>,
}

pub struct FunctionFinderVisitor {
    pub pos: Position,
    pub state: Rc<RefCell<FunctionFinderState>>,
}

impl FunctionFinderVisitor {
    pub fn new(pos: Position) -> Self {
        FunctionFinderVisitor {
            state: Rc::new(RefCell::new(
                FunctionFinderState::default(),
            )),
            pos,
        }
    }
}

fn create_function_result(
    name: String,
    expr: &Expression,
) -> Option<FunctionInfo> {
    if let Expression::Function(f) = expr {
        if let MonoType::Fun(fun) = f.typ.clone() {
            return Some(FunctionInfo::new(
                name,
                fun.as_ref(),
                "self".to_string(),
            ));
        }
    }

    None
}

fn is_before_position(loc: &SourceLocation, pos: Position) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return false;
    }

    true
}

impl<'a> Visitor<'a> for FunctionFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        let loc = node.loc();
        let pos = self.pos.clone();

        if !is_before_position(loc, pos) {
            return true;
        }

        if let Node::VariableAssgn(assgn) = node.as_ref() {
            if let Some(f) = create_function_result(
                assgn.id.name.clone(),
                &assgn.init,
            ) {
                let mut state = self.state.borrow_mut();
                (*state).functions.push(Rc::new(f));
            }
        }
        true
    }
}
