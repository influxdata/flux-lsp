use flux::ast::SourceLocation;
use flux::semantic::nodes::Expression;
use flux::semantic::types::MonoType;
use flux::semantic::walk::{Node, Visitor};
use lspower::lsp;

use crate::completion::CompletionFunction;

fn defined_after(loc: &SourceLocation, pos: lsp::Position) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return true;
    }

    false
}

pub struct FunctionFinderVisitor {
    pub pos: lsp::Position,
    pub functions: Vec<CompletionFunction>,
}

impl FunctionFinderVisitor {
    pub fn new(pos: lsp::Position) -> Self {
        FunctionFinderVisitor {
            pos,
            functions: vec![],
        }
    }
}

impl<'a> Visitor<'a> for FunctionFinderVisitor {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let loc = node.loc();

        if defined_after(loc, self.pos) {
            return true;
        }

        if let Node::VariableAssgn(assgn) = node {
            let name = &assgn.id.name;

            if let Expression::Function(f) = &assgn.init {
                self.functions.push(CompletionFunction::from_expr(
                    name.to_string(),
                    f,
                ));
            }
        }

        if let Node::OptionStmt(opt) = node {
            if let flux::semantic::nodes::Assignment::Variable(
                assgn,
            ) = &opt.assignment
            {
                let name = &assgn.id.name;
                if let Expression::Function(f) = &assgn.init {
                    if let MonoType::Fun(fun) = &f.typ {
                        self.functions.push(CompletionFunction::new(
                            name.to_string(),
                            fun,
                        ));
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
    pub function: CompletionFunction,
}

#[derive(Default)]
pub struct ObjectFunctionFinderVisitor {
    pub results: Vec<ObjectFunction>,
}

impl<'a> Visitor<'a> for ObjectFunctionFinderVisitor {
    fn visit(&mut self, node: Node<'a>) -> bool {
        match node {
            Node::VariableAssgn(assignment) => {
                let object_name = &assignment.id.name;

                if let Expression::Object(obj) = &assignment.init {
                    for prop in &obj.properties {
                        let func_name = &prop.key.name;

                        if let Expression::Function(fun) = &prop.value
                        {
                            self.results.push(ObjectFunction {
                                object: object_name.to_string(),
                                function:
                                    CompletionFunction::from_expr(
                                        func_name.to_string(),
                                        fun,
                                    ),
                            });

                            return false;
                        }
                    }
                }
            }
            Node::OptionStmt(opt) => {
                if let flux::semantic::nodes::Assignment::Variable(
                    assignment,
                ) = &opt.assignment
                {
                    let object_name = &assignment.id.name;
                    if let Expression::Object(obj) = &assignment.init
                    {
                        for prop in &obj.properties {
                            let func_name = &prop.key.name;

                            if let Expression::Function(fun) =
                                &prop.value
                            {
                                self.results.push(ObjectFunction {
                                    object: object_name.to_string(),
                                    function:
                                        CompletionFunction::from_expr(
                                            func_name.to_string(),
                                            fun,
                                        ),
                                });

                                return false;
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
