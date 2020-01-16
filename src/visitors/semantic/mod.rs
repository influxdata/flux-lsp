use std::cell::RefCell;
use std::rc::Rc;

use crate::protocol::properties::Position;
use crate::protocol::responses::{
    CompletionItem, CompletionItemKind, InsertTextFormat,
};
use crate::stdlib::Completable;

// pub mod walk;

use flux::semantic::nodes::*;
use flux::semantic::types::MonoType;
use flux::semantic::walk::{self, Node, Visitor};

mod symbols;
pub mod utils;

pub use symbols::SymbolsVisitor;

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
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
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
    fn visit(&mut self, node: Rc<walk::Node<'a>>) -> bool {
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
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
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

#[derive(Default)]
pub struct FoldFinderState<'a> {
    pub nodes: Vec<Rc<Node<'a>>>,
}

#[derive(Default)]
pub struct FoldFinderVisitor<'a> {
    pub state: Rc<RefCell<FoldFinderState<'a>>>,
}

impl<'a> Visitor<'a> for FoldFinderVisitor<'a> {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        let mut state = self.state.borrow_mut();

        if let Node::Block(_) = node.as_ref() {
            (*state).nodes.push(node.clone());
        }

        true
    }
}

#[derive(Default)]
pub struct ImportFinderState {
    pub imports: Vec<String>,
}

#[derive(Default)]
pub struct ImportFinderVisitor {
    pub state: Rc<RefCell<ImportFinderState>>,
}

impl<'a> Visitor<'a> for ImportFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        let mut state = self.state.borrow_mut();

        if let Node::ImportDeclaration(import) = node.as_ref() {
            (*state).imports.push(import.path.value.clone());
        }

        true
    }
}

#[derive(Default)]
pub struct CompletableFinderState {
    pub completables: Vec<Rc<dyn Completable>>,
}

pub struct CompletableFinderVisitor {
    pub pos: Position,
    pub state: Rc<RefCell<CompletableFinderState>>,
}

impl CompletableFinderVisitor {
    pub fn new(pos: Position) -> Self {
        CompletableFinderVisitor {
            state: Rc::new(RefCell::new(
                CompletableFinderState::default(),
            )),
            pos: pos.clone(),
        }
    }
}

fn follow_function_pipes(c: Box<CallExpr>) -> MonoType {
    if let Some(p) = c.pipe {
        if let Expression::Call(call) = p {
            return follow_function_pipes(call);
        }
    }

    c.typ
}

impl<'a> Visitor<'a> for CompletableFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        let mut state = self.state.borrow_mut();
        let loc = node.loc();
        let pos = self.pos.clone();

        if loc.start.line > pos.line + 1
            || (loc.start.line == pos.line + 1
                && loc.start.column > pos.character + 1)
        {
            return true;
        }

        match node.as_ref() {
            Node::VariableAssgn(assgn) => {
                let id = assgn.id.clone();
                match assgn.init.clone() {
                    Expression::Integer(_) => (*state)
                        .completables
                        .push(Rc::new(VarResult {
                            name: id.name.clone(),
                            var_type: VarType::Int,
                        })),
                    Expression::Boolean(_) => (*state)
                        .completables
                        .push(Rc::new(VarResult {
                            name: id.name.clone(),
                            var_type: VarType::Bool,
                        })),
                    Expression::Float(_) => (*state)
                        .completables
                        .push(Rc::new(VarResult {
                            name: id.name.clone(),
                            var_type: VarType::Float,
                        })),
                    Expression::StringLit(_) => (*state)
                        .completables
                        .push(Rc::new(VarResult {
                            name: id.name.clone(),
                            var_type: VarType::String,
                        })),
                    Expression::Array(_) => (*state)
                        .completables
                        .push(Rc::new(VarResult {
                            name: id.name.clone(),
                            var_type: VarType::Array,
                        })),
                    Expression::Regexp(_) => (*state)
                        .completables
                        .push(Rc::new(VarResult {
                            name: id.name.clone(),
                            var_type: VarType::Regexp,
                        })),
                    Expression::Duration(_) => (*state)
                        .completables
                        .push(Rc::new(VarResult {
                            name: id.name.clone(),
                            var_type: VarType::Duration,
                        })),
                    Expression::Call(c) => {
                        let result_type =
                            follow_function_pipes(c.clone());
                        match result_type {
                            MonoType::Int => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Int,
                                    },
                                ));
                            }
                            MonoType::Float => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Float,
                                    },
                                ));
                            }
                            MonoType::Bool => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Bool,
                                    },
                                ));
                            }
                            MonoType::Arr(_) => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Array,
                                    },
                                ));
                            }
                            MonoType::Duration => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Duration,
                                    },
                                ));
                            }
                            MonoType::Row(_) => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Row,
                                    },
                                ));
                            }
                            MonoType::String => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::String,
                                    },
                                ));
                            }
                            MonoType::Uint => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Uint,
                                    },
                                ));
                            }
                            MonoType::Time => {
                                (*state).completables.push(Rc::new(
                                    VarResult {
                                        name: id.name.clone(),
                                        var_type: VarType::Time,
                                    },
                                ));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        true
    }
}

#[derive(Clone)]
enum VarType {
    Int,
    String,
    Array,
    Float,
    Bool,
    Duration,
    Regexp,
    Row,
    Uint,
    Time,
}

#[derive(Clone)]
struct VarResult {
    pub name: String,
    pub var_type: VarType,
}

impl VarResult {
    pub fn detail(&self) -> String {
        match self.var_type {
            VarType::Array => "Array".to_string(),
            VarType::Bool => "Boolean".to_string(),
            VarType::Duration => "Duration".to_string(),
            VarType::Float => "Float".to_string(),
            VarType::Int => "Integer".to_string(),
            VarType::Regexp => "Regular Expression".to_string(),
            VarType::String => "String".to_string(),
            VarType::Row => "Row".to_string(),
            VarType::Time => "Time".to_string(),
            VarType::Uint => "Unsigned Integer".to_string(),
        }
    }
}

impl Completable for VarResult {
    fn completion_item(&self) -> CompletionItem {
        CompletionItem {
            label: format!("{} ({})", self.name, "self".to_string()),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some(self.detail()),
            documentation: Some("from self".to_string()),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: InsertTextFormat::PlainText,
            kind: Some(CompletionItemKind::Variable),
            preselect: None,
            sort_text: None,
            text_edit: None,
        }
    }

    fn matches(&self, _text: String, _imports: Vec<String>) -> bool {
        true
    }
}
