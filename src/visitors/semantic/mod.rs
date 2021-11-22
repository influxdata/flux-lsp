use std::cell::RefCell;
use std::rc::Rc;

use crate::shared::get_package_name;

use flux::semantic::nodes::Expression;
use flux::semantic::walk::{self, Node, Visitor};
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

pub struct NodeFinderState<'a> {
    pub node: Option<Node<'a>>,
    pub position: lsp::Position,
    pub path: Vec<Node<'a>>,
}

#[derive(Clone)]
pub struct NodeFinderVisitor<'a> {
    pub state: Rc<RefCell<NodeFinderState<'a>>>,
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let mut state = self.state.borrow_mut();

        let contains =
            contains_position(node.clone(), (*state).position);

        if contains {
            (*state).path.push(node.clone());
            (*state).node = Some(node.clone());
        }

        true
    }
}

impl<'a> NodeFinderVisitor<'a> {
    pub fn new(pos: lsp::Position) -> NodeFinderVisitor<'a> {
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
    pub identifiers: Vec<walk::Node<'a>>,
}

#[derive(Clone)]
pub struct IdentFinderVisitor<'a> {
    pub state: Rc<RefCell<IdentFinderState<'a>>>,
}

impl<'a> Visitor<'a> for IdentFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        let mut state = self.state.borrow_mut();
        match node.clone() {
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
    pub node: Option<Node<'a>>,
}

#[derive(Clone)]
pub struct DefinitionFinderVisitor<'a> {
    pub state: Rc<RefCell<DefinitionFinderState<'a>>>,
}

impl<'a> Visitor<'a> for DefinitionFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let mut state = self.state.borrow_mut();

        match node {
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

#[derive(Default)]
pub struct FoldFinderState<'a> {
    pub nodes: Vec<Node<'a>>,
}

#[derive(Default)]
pub struct FoldFinderVisitor<'a> {
    pub state: Rc<RefCell<FoldFinderState<'a>>>,
}

impl<'a> Visitor<'a> for FoldFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let mut state = self.state.borrow_mut();

        if let Node::Block(_) = node {
            (*state).nodes.push(node.clone());
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
pub struct ImportFinderState {
    pub imports: Vec<Import>,
}

#[derive(Default)]
pub struct ImportFinderVisitor {
    pub state: Rc<RefCell<ImportFinderState>>,
}

impl<'a> Visitor<'a> for ImportFinderVisitor {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let mut state = self.state.borrow_mut();

        if let Node::ImportDeclaration(import) = node {
            let alias = match import.alias.clone() {
                Some(alias) => alias.name,
                None => get_package_name(import.path.value.clone())
                    .unwrap_or_else(|| "".to_string()),
            };

            (*state).imports.push(Import {
                path: import.path.value.clone(),
                alias,
                initial_name: get_package_name(
                    import.path.value.clone(),
                ),
            });
        }

        true
    }
}
