use std::cell::RefCell;
use std::rc::Rc;

use crate::protocol::properties::{SymbolInformation, SymbolKind};

use flux::semantic::nodes::{self, Expression};
use flux::semantic::walk::{Node, Visitor};

fn parse_variable_assignment(
    uri: String,
    node: Rc<Node>,
    va: &nodes::VariableAssgn,
) -> Vec<SymbolInformation> {
    let mut result = vec![];

    if let Expression::Function(f) = va.init.clone() {
        result.push(SymbolInformation::new(
            SymbolKind::Function,
            va.id.name.clone(),
            uri.clone(),
            node.loc(),
        ));

        for param in f.params {
            result.push(SymbolInformation::new(
                SymbolKind::Variable,
                param.key.name,
                uri.clone(),
                &param.loc,
            ));
        }
    } else {
        result.push(SymbolInformation::new(
            SymbolKind::Variable,
            va.id.name.clone(),
            uri,
            node.loc(),
        ))
    }

    result
}

fn parse_call_expression(
    uri: String,
    c: &nodes::CallExpr,
) -> Vec<SymbolInformation> {
    let mut result = vec![];

    if let Expression::Identifier(ident) = c.callee.clone() {
        result.push(SymbolInformation::new(
            SymbolKind::Function,
            ident.name,
            uri.clone(),
            &c.loc,
        ))
    }

    for arg in c.arguments.clone() {
        if let Expression::Function(_) = arg.value {
            result.push(SymbolInformation::new(
                SymbolKind::Function,
                arg.key.name.clone(),
                uri.clone(),
                &arg.loc,
            ));
        } else {
            result.push(SymbolInformation::new(
                SymbolKind::Variable,
                arg.key.name.clone(),
                uri.clone(),
                &arg.loc,
            ));
        }
    }

    result
}

fn parse_binary_expression(
    uri: String,
    be: &nodes::BinaryExpr,
) -> Vec<SymbolInformation> {
    let mut result = vec![];

    if let Expression::Identifier(ident) = be.left.clone() {
        result.push(SymbolInformation::new(
            SymbolKind::Variable,
            ident.name.clone(),
            uri.clone(),
            &ident.loc,
        ))
    }

    if let Expression::Identifier(ident) = be.right.clone() {
        result.push(SymbolInformation::new(
            SymbolKind::Variable,
            ident.name.clone(),
            uri,
            &ident.loc,
        ))
    }

    result
}

#[derive(Default)]
pub struct SymbolsState<'a> {
    pub symbols: Vec<SymbolInformation>,
    pub uri: String,
    pub path: Vec<Rc<Node<'a>>>,
}

#[derive(Default)]
pub struct SymbolsVisitor<'a> {
    pub state: Rc<RefCell<SymbolsState<'a>>>,
}

impl<'a> SymbolsVisitor<'a> {
    pub fn new(uri: String) -> SymbolsVisitor<'a> {
        let state = SymbolsState {
            path: vec![],
            symbols: vec![],
            uri,
        };
        SymbolsVisitor {
            state: Rc::new(RefCell::new(state)),
        }
    }
}

impl<'a> SymbolsVisitor<'a> {}

impl<'a> Visitor<'a> for SymbolsVisitor<'a> {
    fn done(&mut self, _: Rc<Node<'a>>) {
        let mut state = self.state.borrow_mut();
        (*state).path.pop();
    }

    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        let mut state = self.state.borrow_mut();
        let uri = (*state).uri.clone();

        (*state).path.push(node.clone());

        match node.as_ref() {
            Node::VariableAssgn(va) => {
                let list =
                    parse_variable_assignment(uri, node.clone(), va);

                for si in list {
                    (*state).symbols.push(si);
                }
            }
            Node::CallExpr(c) => {
                let list = parse_call_expression(uri, c);

                for si in list {
                    (*state).symbols.push(si);
                }
            }
            Node::BinaryExpr(be) => {
                let list = parse_binary_expression(uri, be);

                for si in list {
                    (*state).symbols.push(si);
                }
            }
            Node::MemberExpr(me) => {
                if let Some(source) = me.loc.source.clone() {
                    (*state).symbols.push(SymbolInformation::new(
                        SymbolKind::Object,
                        source,
                        uri,
                        &me.loc,
                    ))
                }
            }
            Node::FloatLit(num) => {
                (*state).symbols.push(SymbolInformation::new(
                    SymbolKind::Number,
                    num.value.to_string(),
                    uri,
                    &num.loc,
                ));
                return false;
            }
            Node::IntegerLit(num) => {
                (*state).symbols.push(SymbolInformation::new(
                    SymbolKind::Number,
                    num.value.to_string(),
                    uri,
                    &num.loc,
                ));
                return false;
            }
            Node::DateTimeLit(d) => {
                (*state).symbols.push(SymbolInformation::new(
                    SymbolKind::Constant,
                    d.value.to_string(),
                    uri,
                    &d.loc,
                ));
                return false;
            }
            Node::BooleanLit(b) => {
                (*state).symbols.push(SymbolInformation::new(
                    SymbolKind::Boolean,
                    b.value.to_string(),
                    uri,
                    &b.loc,
                ));
                return false;
            }
            Node::StringLit(s) => {
                (*state).symbols.push(SymbolInformation::new(
                    SymbolKind::String,
                    s.value.clone(),
                    uri,
                    &s.loc,
                ));
                return false;
            }
            Node::ArrayExpr(a) => {
                (*state).symbols.push(SymbolInformation::new(
                    SymbolKind::Array,
                    String::from("[]"),
                    uri,
                    &a.loc,
                ))
            }
            _ => (),
        }
        true
    }
}
