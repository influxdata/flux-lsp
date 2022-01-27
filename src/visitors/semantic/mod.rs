use crate::shared::get_package_name;

use flux::semantic::{
    nodes::{Expression, Symbol},
    walk::{self, Node, Visitor},
};
use lspower::lsp;

mod completion;
mod symbols;

mod functions;

pub use completion::{
    FunctionFinderVisitor, ObjectFunctionFinderVisitor,
};
pub use symbols::SymbolsVisitor;

fn contains_position(node: Node<'_>, pos: lsp::Position) -> bool {
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

#[derive(Debug)]
pub struct NodeFinderVisitor<'a> {
    pub node: Option<Node<'a>>,
    pub position: lsp::Position,
    pub path: Vec<Node<'a>>,
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let contains = contains_position(node.clone(), self.position);

        if contains {
            self.path.push(node.clone());
            self.node = Some(node.clone());
        }

        true
    }
}

impl<'a> NodeFinderVisitor<'a> {
    pub fn new(pos: lsp::Position) -> NodeFinderVisitor<'a> {
        NodeFinderVisitor {
            node: None,
            position: pos,
            path: vec![],
        }
    }
}

pub struct IdentFinderVisitor<'a> {
    pub name: Symbol,
    pub identifiers: Vec<walk::Node<'a>>,
}

impl<'a> Visitor<'a> for IdentFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        match node.clone() {
            walk::Node::MemberExpr(m) => {
                if let Expression::Identifier(i) = &m.object {
                    if i.name == self.name {
                        return true;
                    }
                }
                return false;
            }
            walk::Node::Identifier(n) => {
                if n.name == self.name {
                    self.identifiers.push(node.clone());
                }
            }
            walk::Node::IdentifierExpr(n) => {
                if n.name == self.name {
                    self.identifiers.push(node.clone());
                }
            }
            _ => {}
        }
        true
    }
}

impl<'a> IdentFinderVisitor<'a> {
    pub fn new(name: Symbol) -> IdentFinderVisitor<'a> {
        IdentFinderVisitor {
            name,
            identifiers: vec![],
        }
    }
}

pub struct DefinitionFinderVisitor<'a> {
    pub name: Symbol,
    pub node: Option<Node<'a>>,
}

impl<'a> Visitor<'a> for DefinitionFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        match node {
            walk::Node::VariableAssgn(v) => {
                if v.id.name == self.name {
                    self.node = Some(node);
                    return false;
                }

                true
            }
            walk::Node::BuiltinStmt(v) => {
                if v.id.name == self.name {
                    self.node = Some(walk::Node::Identifier(&v.id));
                    return false;
                }

                true
            }
            walk::Node::FunctionParameter(param) => {
                if param.key.name == self.name {
                    self.node = Some(node);
                    return false;
                }

                true
            }
            _ => true,
        }
    }
}

impl<'a> DefinitionFinderVisitor<'a> {
    pub fn new(name: Symbol) -> DefinitionFinderVisitor<'a> {
        DefinitionFinderVisitor { name, node: None }
    }
}

#[derive(Default)]
pub struct FoldFinderVisitor<'a> {
    pub nodes: Vec<Node<'a>>,
}

impl<'a> Visitor<'a> for FoldFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        if let Node::Block(_) = node {
            self.nodes.push(node.clone());
        }

        true
    }
}

#[derive(Clone)]
pub struct Import {
    pub path: String,
    pub initial_name: Option<String>,
    pub alias: String,
}

#[derive(Default)]
pub struct ImportFinderVisitor {
    pub imports: Vec<Import>,
}

impl<'a> Visitor<'a> for ImportFinderVisitor {
    fn visit(&mut self, node: Node<'a>) -> bool {
        if let Node::ImportDeclaration(import) = node {
            let alias = match import.alias.clone() {
                Some(alias) => alias.name.to_string(),
                None => get_package_name(import.path.value.as_str())
                    .unwrap_or_else(|| "".to_string()),
            };

            self.imports.push(Import {
                path: import.path.value.clone(),
                alias,
                initial_name: get_package_name(
                    import.path.value.as_str(),
                ),
            });
        }

        true
    }
}
