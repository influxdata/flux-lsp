use std::cell::RefCell;
use std::rc::Rc;

use flux::ast::walk::{self, Node, Visitor};
use lsp_types as lsp;

use crate::shared::conversion::flux_position_to_position;

fn contains_position(
    node: Rc<walk::Node<'_>>,
    pos: lsp::Position,
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
    pub position: lsp::Position,
}

impl<'a> CallFinderVisitor<'a> {
    pub fn new(position: lsp::Position) -> Self {
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

        let contains = contains_position(node.clone(), self.position);

        if contains {
            if let walk::Node::CallExpr(_) = node.as_ref() {
                (*state).node = Some(node.clone())
            }
        }

        Some(self.clone())
    }
}

#[derive(Clone)]
pub struct NodeFinderNode<'a> {
    pub node: Rc<walk::Node<'a>>,
    pub parent: Option<Box<NodeFinderNode<'a>>>,
}

pub struct NodeFinderState<'a> {
    pub node: Option<NodeFinderNode<'a>>,
    pub position: lsp::Position,
}

impl<'a> NodeFinderState<'a> {}

#[derive(Clone)]
pub struct NodeFinderVisitor<'a> {
    pub state: Rc<RefCell<NodeFinderState<'a>>>,
}

impl<'a> NodeFinderVisitor<'a> {
    pub fn new(position: lsp::Position) -> Self {
        NodeFinderVisitor {
            state: Rc::new(RefCell::new(NodeFinderState {
                node: None,
                position,
            })),
        }
    }
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&self, node: Rc<walk::Node<'a>>) -> Option<Self> {
        let mut state = self.state.borrow_mut();

        let contains =
            contains_position(node.clone(), (*state).position);

        if contains {
            let parent = (*state).node.clone();
            if let Some(parent) = parent {
                (*state).node = Some(NodeFinderNode {
                    node: node.clone(),
                    parent: Some(Box::new(parent)),
                });
            } else {
                (*state).node = Some(NodeFinderNode {
                    node: node.clone(),
                    parent: None,
                });
            }
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

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub position: lsp::Position,
}

#[derive(Default)]
pub struct PackageFinderState {
    pub info: Option<PackageInfo>,
}

#[derive(Clone, Default)]
pub struct PackageFinderVisitor {
    pub state: Rc<RefCell<PackageFinderState>>,
}

impl<'a> Visitor<'a> for PackageFinderVisitor {
    fn visit(&self, node: Rc<Node<'a>>) -> Option<Self> {
        if let Node::PackageClause(p) = node.as_ref() {
            let mut state = self.state.borrow_mut();
            state.info = Some(PackageInfo {
                name: "".to_string(),
                position: flux_position_to_position(
                    p.base.location.clone().start,
                ),
            })
        }
        Some(self.clone())
    }
}
