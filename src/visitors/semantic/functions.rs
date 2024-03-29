use std::rc::Rc;

use flux::ast::SourceLocation;
use flux::semantic::nodes::Expression;
use flux::semantic::types::MonoType;
use flux::semantic::walk::{Node, Visitor};
use lspower::lsp;

pub struct FunctionInfo {
    pub name: String,
    pub package_name: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

impl FunctionInfo {
    pub fn new(
        name: String,
        f: &flux::semantic::types::Function,
        package_name: String,
    ) -> Self {
        FunctionInfo {
            name,
            package_name,
            required_args: f.req.keys().map(String::from).collect(),
            optional_args: f.opt.keys().map(String::from).collect(),
        }
    }
}

pub struct FunctionFinderVisitor {
    pub pos: lsp::Position,
    pub functions: Vec<Rc<FunctionInfo>>,
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

fn is_before_position(
    loc: &SourceLocation,
    pos: lsp::Position,
) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return false;
    }

    true
}

impl<'a> Visitor<'a> for FunctionFinderVisitor {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let loc = node.loc();
        let pos = self.pos;

        if !is_before_position(loc, pos) {
            return true;
        }

        if let Node::VariableAssgn(assgn) = node {
            if let Some(f) = create_function_result(
                assgn.id.name.to_string(),
                &assgn.init,
            ) {
                self.functions.push(Rc::new(f));
            }
        }
        true
    }
}
