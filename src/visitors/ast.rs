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
pub struct CallFinderVisitor<'a> {
    pub node: Option<walk::Node<'a>>,
    pub position: lsp::Position,
}

impl<'a> CallFinderVisitor<'a> {
    pub fn new(position: lsp::Position) -> Self {
        CallFinderVisitor {
            node: None,
            position,
        }
    }
}

impl<'a> Visitor<'a> for CallFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        let contains = contains_position(node.clone(), self.position);

        if contains {
            if let walk::Node::CallExpr(_) = node {
                self.node = Some(node.clone())
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
pub struct NodeFinderVisitor<'a> {
    pub node: Option<NodeFinderNode<'a>>,
    pub position: lsp::Position,
}

impl<'a> NodeFinderVisitor<'a> {
    pub fn new(position: lsp::Position) -> Self {
        NodeFinderVisitor {
            node: None,
            position,
        }
    }
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        let contains = contains_position(node.clone(), self.position);

        if contains {
            let parent = self.node.clone();
            if let Some(parent) = parent {
                self.node = Some(NodeFinderNode {
                    node: node.clone(),
                    parent: Some(Box::new(parent)),
                });
            } else {
                self.node =
                    Some(NodeFinderNode { node, parent: None });
            }
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
