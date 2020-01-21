use std::cell::RefCell;
use std::rc::Rc;

use crate::protocol::properties::Position;
use crate::protocol::responses::{
    CompletionItem, CompletionItemKind, InsertTextFormat,
};
use crate::stdlib::Completable;
use crate::visitors::semantic::utils;

use flux::semantic::nodes::*;
use flux::semantic::types::MonoType;
use flux::semantic::walk::{Node, Visitor};

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
            pos,
        }
    }
}

fn get_var_type(expr: &Expression) -> Option<VarType> {
    match expr {
        Expression::Integer(_) => Some(VarType::Int),
        Expression::Boolean(_) => Some(VarType::Bool),
        Expression::Float(_) => Some(VarType::Float),
        Expression::StringLit(_) => Some(VarType::String),
        Expression::Array(_) => Some(VarType::Array),
        Expression::Regexp(_) => Some(VarType::Regexp),
        Expression::Duration(_) => Some(VarType::Duration),
        Expression::Call(c) => {
            let result_type = utils::follow_function_pipes(c);

            match result_type {
                MonoType::Int => Some(VarType::Int),
                MonoType::Float => Some(VarType::Float),
                MonoType::Bool => Some(VarType::Bool),
                MonoType::Arr(_) => Some(VarType::Array),
                MonoType::Duration => Some(VarType::Duration),
                MonoType::Row(_) => Some(VarType::Row),
                MonoType::String => Some(VarType::String),
                MonoType::Uint => Some(VarType::Uint),
                MonoType::Time => Some(VarType::Time),
                _ => None,
            }
        }
        _ => None,
    }
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

        if let Node::VariableAssgn(assgn) = node.as_ref() {
            if let Some(var_type) = get_var_type(&assgn.init) {
                let name = assgn.id.name.clone();
                (*state)
                    .completables
                    .push(Rc::new(VarResult { var_type, name }))
            }
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
