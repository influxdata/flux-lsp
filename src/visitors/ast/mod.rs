use crate::protocol::properties::Position;

use std::cell::RefCell;
use std::rc::Rc;

use flux::ast::walk::{self, Visitor};

pub fn contains_line_ref(
    node: &walk::Node<'_>,
    pos: &Position,
) -> bool {
    let start_line = node.base().location.start.line - 1;
    let end_line = node.base().location.end.line - 1;
    if pos.line < start_line {
        return false;
    }

    if pos.line > end_line {
        return false;
    }
    true
}

// TODO: figure out if this offset is common among lsp clients
fn contains_position(
    node: Rc<walk::Node<'_>>,
    pos: Position,
) -> bool {
    let start_line = node.base().location.start.line - 1;
    let start_col = node.base().location.start.column - 1;
    let end_line = node.base().location.end.line - 1;
    let end_col = node.base().location.end.column - 1;

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

pub struct DefinitionFinderState<'a> {
    pub name: String,
    pub node: Option<Rc<walk::Node<'a>>>,
}

#[derive(Clone)]
pub struct DefinitionFinderVisitor<'a> {
    pub state: Rc<RefCell<DefinitionFinderState<'a>>>,
}

impl<'a> Visitor<'a> for DefinitionFinderVisitor<'a> {
    fn visit(&self, node: Rc<walk::Node<'a>>) -> Option<Self> {
        let mut state = self.state.borrow_mut();

        match node.as_ref() {
            walk::Node::VariableAssgn(v) => {
                if v.id.name == state.name {
                    state.node = Some(node.clone());
                    return None;
                }

                Some(self.clone())
            }
            walk::Node::FunctionExpr(_) => None,
            _ => Some(self.clone()),
        }
    }
}

pub struct CallFinderState<'a> {
    pub node: Option<Rc<walk::Node<'a>>>,
}

#[derive(Clone)]
pub struct CallFinderVisitor<'a> {
    pub state: Rc<RefCell<CallFinderState<'a>>>,
    pub position: Position,
}

impl<'a> CallFinderVisitor<'a> {
    pub fn new(position: Position) -> Self {
        CallFinderVisitor {
            state: Rc::new(RefCell::new(CallFinderState {
                node: None,
            })),
            position,
        }
    }
}

impl<'a> Visitor<'a> for CallFinderVisitor<'a> {
    fn visit(&self, node: Rc<walk::Node<'a>>) -> Option<Self> {
        let mut state = self.state.borrow_mut();

        let contains =
            contains_position(node.clone(), self.position.clone());

        if contains {
            if let walk::Node::CallExpr(_) = node.as_ref() {
                (*state).node = Some(node.clone())
            }
        }

        Some(self.clone())
    }
}

pub struct NodeFinderState<'a> {
    pub node: Option<Rc<walk::Node<'a>>>,
    pub position: Position,
    pub path: Vec<Rc<walk::Node<'a>>>,
}

#[derive(Clone)]
pub struct NodeFinderVisitor<'a> {
    pub state: Rc<RefCell<NodeFinderState<'a>>>,
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&self, node: Rc<walk::Node<'a>>) -> Option<Self> {
        let mut state = self.state.borrow_mut();

        let contains = contains_position(
            node.clone(),
            (*state).position.clone(),
        );

        if contains {
            (*state).path.push(node.clone());
            (*state).node = Some(node.clone());
        }

        Some(self.clone())
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
    fn visit(&self, node: Rc<walk::Node<'a>>) -> Option<Self> {
        let mut state = self.state.borrow_mut();
        match node.clone().as_ref() {
            walk::Node::MemberExpr(m) => {
                if let flux::ast::Expression::Identifier(i) =
                    m.object.clone()
                {
                    if i.name == state.name {
                        return Some(self.clone());
                    }
                }
                return None;
            }
            walk::Node::Identifier(n) => {
                if n.name == state.name {
                    state.identifiers.push(node.clone());
                }
            }
            _ => {}
        }
        Some(self.clone())
    }
}
