use flux::ast::{
    walk::{self, Visitor},
    Package,
};
use lspower::lsp;

use crate::shared::flux_position_to_position;

fn contains_position(
    node: walk::Node<'_>,
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

#[derive(Clone)]
pub struct CallFinderState<'a> {
    pub node: Option<walk::Node<'a>>,
}

#[derive(Clone)]
pub struct CallFinderVisitor<'a> {
    pub state: CallFinderState<'a>,
    pub position: lsp::Position,
}

impl<'a> CallFinderVisitor<'a> {
    pub fn new(position: lsp::Position) -> Self {
        CallFinderVisitor {
            state: CallFinderState { node: None },
            position,
        }
    }
}

impl<'a> Visitor<'a> for CallFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        let contains = contains_position(node.clone(), self.position);

        if contains {
            if let walk::Node::CallExpr(_) = node {
                self.state.node = Some(node.clone())
            }
        }

        true
    }
}

#[derive(Clone)]
pub struct NodeFinderNode<'a> {
    pub node: walk::Node<'a>,
    pub parent: Option<Box<NodeFinderNode<'a>>>,
}

#[derive(Clone)]
pub struct NodeFinderState<'a> {
    pub node: Option<NodeFinderNode<'a>>,
    pub position: lsp::Position,
}

#[derive(Clone)]
pub struct NodeFinderVisitor<'a> {
    pub state: NodeFinderState<'a>,
}

impl<'a> NodeFinderVisitor<'a> {
    pub fn new(position: lsp::Position) -> Self {
        NodeFinderVisitor {
            state: NodeFinderState {
                node: None,
                position,
            },
        }
    }
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        let contains =
            contains_position(node.clone(), self.state.position);

        if contains {
            let parent = self.state.node.clone();
            if let Some(parent) = parent {
                self.state.node = Some(NodeFinderNode {
                    node: node.clone(),
                    parent: Some(Box::new(parent)),
                });
            } else {
                self.state.node =
                    Some(NodeFinderNode { node, parent: None });
            }
        }

        true
    }
}

#[derive(Clone)]
pub struct IdentFinderState<'a> {
    pub name: String,
    pub identifiers: Vec<walk::Node<'a>>,
}

#[derive(Clone)]
pub struct IdentFinderVisitor<'a> {
    pub state: IdentFinderState<'a>,
}

impl<'a> Visitor<'a> for IdentFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        match node {
            walk::Node::MemberExpr(m) => {
                if let flux::ast::Expression::Identifier(i) =
                    m.object.clone()
                {
                    if i.name == self.state.name {
                        return true;
                    }
                }
                return false;
            }
            walk::Node::Identifier(n) => {
                if n.name == self.state.name {
                    self.state.identifiers.push(node);
                }
            }
            _ => {}
        }
        true
    }
}

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub position: lsp::Position,
}

impl From<&Package> for PackageInfo {
    fn from(pkg: &Package) -> Self {
        Self {
            name: pkg.package.clone(),
            position: flux_position_to_position(
                &pkg.base.location.start,
            ),
        }
    }
}
