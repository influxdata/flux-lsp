use std::cell::RefCell;
use std::rc::Rc;

use crate::protocol::properties::Position;
use crate::protocol::responses::{
    CompletionItem, CompletionItemKind, InsertTextFormat,
};
use crate::shared::signatures::get_argument_names;
use crate::stdlib::{create_function_signature, Completable};
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

fn create_function_result(
    name: String,
    expr: &Expression,
) -> Option<FunctionResult> {
    if let Expression::Function(f) = expr {
        if let MonoType::Fun(fun) = f.typ.clone() {
            return Some(FunctionResult {
                name,
                package: "self".to_string(),
                package_name: Some("self".to_string()),
                optional_args: get_argument_names(fun.clone().opt),
                required_args: get_argument_names(fun.clone().req),
                signature: create_function_signature((*fun).clone()),
            });
        }
    }

    None
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
            let name = assgn.id.name.clone();
            if let Some(var_type) = get_var_type(&assgn.init) {
                (*state).completables.push(Rc::new(VarResult {
                    var_type,
                    name: name.clone(),
                }));
            }

            if let Some(fun) =
                create_function_result(name, &assgn.init)
            {
                (*state).completables.push(Rc::new(fun));
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
            sort_text: Some(self.name.clone()),
            text_edit: None,
        }
    }

    fn matches(&self, _text: String, _imports: Vec<String>) -> bool {
        true
    }
}

#[derive(Clone)]
pub struct FunctionResult {
    pub name: String,
    pub package: String,
    pub package_name: Option<String>,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
    pub signature: String,
}

impl FunctionResult {
    fn insert_text(&self) -> String {
        let mut insert_text = format!("{}(", self.name);

        for (index, arg) in self.required_args.iter().enumerate() {
            insert_text +=
                (format!("{}: ${}", arg, index + 1)).as_str();

            if index != self.required_args.len() - 1 {
                insert_text += ", ";
            }
        }

        if self.required_args.is_empty()
            && !self.optional_args.is_empty()
        {
            insert_text += "$1";
        }

        insert_text += ")$0";

        insert_text
    }
}

impl Completable for FunctionResult {
    fn completion_item(&self) -> CompletionItem {
        CompletionItem {
            label: format!("{} ({})", self.name, "self".to_string()),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some(self.signature.clone()),
            documentation: Some("from self".to_string()),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.insert_text()),
            insert_text_format: InsertTextFormat::Snippet,
            kind: Some(CompletionItemKind::Function),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
        }
    }

    fn matches(&self, _text: String, _imports: Vec<String>) -> bool {
        true
    }
}
