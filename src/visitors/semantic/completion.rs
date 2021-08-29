use std::rc::Rc;
use std::sync::{Arc, Mutex};

use flux::ast::SourceLocation;
use flux::semantic::nodes::*;
use flux::semantic::types::MonoType;
use flux::semantic::walk::{Node, Visitor};
use lsp_types as lsp;

use crate::shared::signatures::get_argument_names;
use crate::shared::Function;

fn defined_after(loc: &SourceLocation, pos: lsp::Position) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return true;
    }

    false
}

pub struct FunctionFinderState {
    pub functions: Vec<Function>,
}

pub struct FunctionFinderVisitor {
    pub pos: lsp::Position,
    pub state: Arc<Mutex<FunctionFinderState>>,
}

impl FunctionFinderVisitor {
    pub fn new(pos: lsp::Position) -> Self {
        FunctionFinderVisitor {
            pos,
            state: Arc::new(Mutex::new(FunctionFinderState {
                functions: vec![],
            })),
        }
    }
}

impl<'a> Visitor<'a> for FunctionFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let loc = node.loc();

            if defined_after(loc, self.pos) {
                return true;
            }

            if let Node::VariableAssgn(assgn) = node.as_ref() {
                let name = assgn.id.name.clone();

                if let Expression::Function(f) = assgn.init.clone() {
                    if let MonoType::Fun(fun) = f.typ.clone() {
                        let mut params = get_argument_names(fun.req);
                        for opt in get_argument_names(fun.opt) {
                            params.push(opt);
                        }

                        state
                            .functions
                            .push(Function { name, params })
                    }
                }
            }

            if let Node::OptionStmt(opt) = node.as_ref() {
                if let flux::semantic::nodes::Assignment::Variable(
                    assgn,
                ) = &opt.assignment
                {
                    let name = assgn.id.name.clone();
                    if let Expression::Function(f) =
                        assgn.init.clone()
                    {
                        if let MonoType::Fun(fun) = f.typ.clone() {
                            let mut params =
                                get_argument_names(fun.req);
                            for opt in get_argument_names(fun.opt) {
                                params.push(opt);
                            }

                            state
                                .functions
                                .push(Function { name, params })
                        }
                    }
                }
            }
        }

        true
    }
}

#[derive(Clone)]
pub struct ObjectFunction {
    pub object: String,
    pub function: Function,
}

#[derive(Default)]
pub struct ObjectFunctionFinderState {
    pub results: Vec<ObjectFunction>,
}

#[derive(Default)]
pub struct ObjectFunctionFinderVisitor {
    pub state: Arc<Mutex<ObjectFunctionFinderState>>,
}

impl<'a> Visitor<'a> for ObjectFunctionFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        match node.as_ref() {
            Node::VariableAssgn(assignment) => {
                let object_name = assignment.id.name.clone();

                if let Expression::Object(obj) =
                    assignment.init.clone()
                {
                    for prop in obj.properties.clone() {
                        let func_name = prop.key.name;

                        if let Expression::Function(fun) = prop.value
                        {
                            let params = fun
                                .params
                                .into_iter()
                                .map(|p| p.key.name)
                                .collect::<Vec<String>>();

                            if let Ok(mut state) = self.state.lock() {
                                state.results.push(ObjectFunction {
                                    object: object_name,
                                    function: Function {
                                        name: func_name,
                                        params,
                                    },
                                });

                                return false;
                            }
                        }
                    }
                }
            }
            Node::OptionStmt(opt) => {
                if let flux::semantic::nodes::Assignment::Variable(
                    assignment,
                ) = opt.assignment.clone()
                {
                    let object_name = assignment.id.name;
                    if let Expression::Object(obj) = assignment.init {
                        for prop in obj.properties.clone() {
                            let func_name = prop.key.name;

                            if let Expression::Function(fun) =
                                prop.value
                            {
                                let params = fun
                                    .params
                                    .into_iter()
                                    .map(|p| p.key.name)
                                    .collect::<Vec<String>>();

                                if let Ok(mut state) =
                                    self.state.lock()
                                {
                                    state.results.push(
                                        ObjectFunction {
                                            object: object_name,
                                            function: Function {
                                                name: func_name,
                                                params,
                                            },
                                        },
                                    );

                                    return false;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        true
    }
}
