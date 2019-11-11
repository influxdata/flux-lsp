use std::cell::RefCell;
use std::rc::Rc;

use crate::protocol::properties::Position;

pub mod walk;
use walk::{Node, Visitor};

use flux::semantic::nodes::Expression;

pub mod utils;

fn contains_position(node: Rc<Node<'_>>, pos: Position) -> bool {
    let start_line = node.loc().start.line - 1;
    let start_col = node.loc().start.column - 1;
    let end_line = node.loc().end.line - 1;
    let end_col = node.loc().end.column - 1;

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

pub struct NodeFinderState<'a> {
    pub node: Option<Rc<Node<'a>>>,
    pub position: Position,
    pub path: Vec<Rc<Node<'a>>>,
}

#[derive(Clone)]
pub struct NodeFinderVisitor<'a> {
    pub state: Rc<RefCell<NodeFinderState<'a>>>,
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&self, node: Rc<Node<'a>>) -> bool {
        let mut state = self.state.borrow_mut();

        let contains = contains_position(
            node.clone(),
            (*state).position.clone(),
        );

        if contains {
            (*state).path.push(node.clone());
            (*state).node = Some(node.clone());
        }

        true
    }
}

impl<'a> NodeFinderVisitor<'a> {
    pub fn new(pos: Position) -> NodeFinderVisitor<'a> {
        NodeFinderVisitor {
            state: Rc::new(RefCell::new(NodeFinderState {
                node: None,
                position: pos,
                path: vec![],
            })),
        }
    }
}

pub struct IdentFinderState<'a> {
    pub name: String,
    pub identifiers: Vec<Rc<walk::Node<'a>>>,
}

#[derive(Clone)]
pub struct IdentFinderVisitor<'a> {
    pub state: Rc<RefCell<IdentFinderState<'a>>>,
}

impl<'a> Visitor<'a> for IdentFinderVisitor<'a> {
    fn visit(&self, node: Rc<walk::Node<'a>>) -> bool {
        let mut state = self.state.borrow_mut();
        match node.clone().as_ref() {
            walk::Node::MemberExpr(m) => {
                if let Expression::Identifier(i) = m.object.clone() {
                    if i.name == state.name {
                        return true;
                    }
                }
                return false;
            }
            walk::Node::Identifier(n) => {
                if n.name == state.name {
                    state.identifiers.push(node.clone());
                }
            }
            walk::Node::IdentifierExpr(n) => {
                if n.name == state.name {
                    state.identifiers.push(node.clone());
                }
            }
            _ => {}
        }
        true
    }
}

impl<'a> IdentFinderVisitor<'a> {
    pub fn new(name: String) -> IdentFinderVisitor<'a> {
        IdentFinderVisitor {
            state: Rc::new(RefCell::new(IdentFinderState {
                name,
                identifiers: vec![],
            })),
        }
    }
}

pub struct DefinitionFinderState<'a> {
    pub name: String,
    pub node: Option<Rc<Node<'a>>>,
}

#[derive(Clone)]
pub struct DefinitionFinderVisitor<'a> {
    pub state: Rc<RefCell<DefinitionFinderState<'a>>>,
}

impl<'a> Visitor<'a> for DefinitionFinderVisitor<'a> {
    fn visit(&self, node: Rc<Node<'a>>) -> bool {
        let mut state = self.state.borrow_mut();

        match node.as_ref() {
            walk::Node::VariableAssgn(v) => {
                if v.id.name == state.name {
                    state.node = Some(node.clone());
                    return false;
                }

                true
            }
            walk::Node::FunctionExpr(_) => false,
            _ => true,
        }
    }
}

impl<'a> DefinitionFinderVisitor<'a> {
    pub fn new(name: String) -> DefinitionFinderVisitor<'a> {
        DefinitionFinderVisitor {
            state: Rc::new(RefCell::new(DefinitionFinderState {
                name,
                node: None,
            })),
        }
    }
}
