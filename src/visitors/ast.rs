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

pub(crate) const SEMANTIC_TOKEN_KEYWORD: u32 = 0;
pub(crate) const SEMANTIC_TOKEN_NUMBER: u32 = 1;
pub(crate) const SEMANTIC_TOKEN_STRING: u32 = 2;

#[derive(Clone, Default)]
pub struct SemanticTokenVisitor {
    pub tokens: Vec<lsp::SemanticToken>,
}

impl<'a> Visitor<'a> for SemanticTokenVisitor {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        match node {
            walk::Node::PackageClause(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = 7; // Length of "package"
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_KEYWORD,
                    token_modifiers_bitset: 0,
                });

                // Identifier itself is just too broad. Manually handle
                // the case for the string identifier.
                self.tokens.push(lsp::SemanticToken {
                    delta_line: node.name.base.location.start.line,
                    delta_start: node.name.base.location.start.column,
                    length: node.name.base.location.end.column
                        - node.name.base.location.start.column,
                    token_type: SEMANTIC_TOKEN_STRING,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::ImportDeclaration(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = 6; // Length of "import"
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_KEYWORD,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::IntegerLit(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = node.base.location.end.column
                    - node.base.location.start.column;
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_NUMBER,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::FloatLit(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = node.base.location.end.column
                    - node.base.location.start.column;
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_NUMBER,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::StringLit(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = node.base.location.end.column
                    - node.base.location.start.column;
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_STRING,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::DurationLit(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = node.base.location.end.column
                    - node.base.location.start.column;
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_NUMBER,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::UintLit(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = node.base.location.end.column
                    - node.base.location.start.column;
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_NUMBER,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::DateTimeLit(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = node.base.location.end.column
                    - node.base.location.start.column;
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_NUMBER,
                    token_modifiers_bitset: 0,
                });
            }
            // These statements are internal to flux developement.
            walk::Node::TestCaseStmt(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = 7; // Length of "builtin"
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_KEYWORD,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::TestStmt(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = 4; // Length of "test"
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_KEYWORD,
                    token_modifiers_bitset: 0,
                });
            }
            walk::Node::BuiltinStmt(node) => {
                let delta_line = node.base.location.start.line;
                let delta_start = node.base.location.start.column;
                let length = 8; // Length of "testcase"
                self.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type: SEMANTIC_TOKEN_KEYWORD,
                    token_modifiers_bitset: 0,
                });
            }
            _ => {
                println!("{}", node);
            }
        }
        true
    }
}
