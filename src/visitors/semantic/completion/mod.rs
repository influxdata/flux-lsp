use std::rc::Rc;

use std::sync::{Arc, Mutex};

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
    pub completables: Vec<Arc<dyn Completable + Send + Sync>>,
}

pub struct CompletableFinderVisitor {
    pub pos: Position,
    pub state: Arc<Mutex<CompletableFinderState>>,
}

impl<'a> Visitor<'a> for CompletableFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        if let Ok(mut state) = self.state.lock() {
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
                    (*state).completables.push(Arc::new(VarResult {
                        var_type,
                        name: name.clone(),
                    }));
                }

                if let Some(fun) =
                    create_function_result(name, &assgn.init)
                {
                    (*state).completables.push(Arc::new(fun));
                }
            }

            if let Node::OptionStmt(opt) = node.as_ref() {
                if let flux::semantic::nodes::Assignment::Variable(
                    var_assign,
                ) = &opt.assignment
                {
                    let name = var_assign.id.name.clone();
                    if let Some(var_type) =
                        get_var_type(&var_assign.init)
                    {
                        (*state).completables.push(Arc::new(
                            VarResult { var_type, name },
                        ));

                        return false;
                    }

                    if let Some(fun) =
                        create_function_result(name, &var_assign.init)
                    {
                        (*state).completables.push(Arc::new(fun));
                        return false;
                    }
                }
            }
        }

        true
    }
}

impl CompletableFinderVisitor {
    pub fn new(pos: Position) -> Self {
        CompletableFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableFinderState::default(),
            )),
            pos,
        }
    }
}

#[derive(Default)]
pub struct CompletableObjectFinderState {
    pub completables: Vec<Arc<dyn Completable + Send + Sync>>,
}

pub struct CompletableObjectFinderVisitor {
    pub name: String,
    pub state: Arc<Mutex<CompletableObjectFinderState>>,
}

impl CompletableObjectFinderVisitor {
    pub fn new(name: String) -> Self {
        CompletableObjectFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableObjectFinderState::default(),
            )),
            name,
        }
    }
}

impl<'a> Visitor<'a> for CompletableObjectFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let name = self.name.clone();

            if let Node::ObjectExpr(obj) = node.as_ref() {
                if let Some(ident) = &obj.with {
                    if name == ident.name {
                        for prop in obj.properties.clone() {
                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    VarResult {
                                        var_type,
                                        name: prop.key.name,
                                    },
                                ));
                            }
                        }
                    }
                }
            }

            if let Node::VariableAssgn(assign) = node.as_ref() {
                if assign.id.name == name {
                    if let Expression::Object(obj) = &assign.init {
                        for prop in obj.properties.clone() {
                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    VarResult {
                                        var_type,
                                        name: prop.key.name,
                                    },
                                ));
                            }
                        }

                        return false;
                    }
                }
            }

            if let Node::OptionStmt(opt) = node.as_ref() {
                if let flux::semantic::nodes::Assignment::Variable(
                    assign,
                ) = opt.assignment.clone()
                {
                    if assign.id.name == name {
                        if let Expression::Object(obj) = assign.init {
                            for prop in obj.properties.clone() {
                                if let Some(var_type) =
                                    get_var_type(&prop.value)
                                {
                                    (*state).completables.push(
                                        Arc::new(VarResult {
                                            var_type,
                                            name: prop.key.name,
                                        }),
                                    );
                                }
                            }

                            return false;
                        }
                    }
                }
            }
        }

        true
    }
}

fn get_var_type(expr: &Expression) -> Option<VarType> {
    match expr.type_of() {
        MonoType::Duration => return Some(VarType::Duration),
        MonoType::Int => return Some(VarType::Int),
        MonoType::Bool => return Some(VarType::Bool),
        MonoType::Float => return Some(VarType::Float),
        MonoType::String => return Some(VarType::String),
        MonoType::Arr(_) => return Some(VarType::Array),
        MonoType::Regexp => return Some(VarType::Regexp),
        _ => {}
    }

    match expr {
        Expression::Object(_) => Some(VarType::Object),
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

#[derive(Clone)]
enum VarType {
    Int,
    String,
    Array,
    Float,
    Bool,
    Duration,
    Object,
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
            VarType::Object => "Object".to_string(),
            VarType::Regexp => "Regular Expression".to_string(),
            VarType::String => "String".to_string(),
            VarType::Row => "Row".to_string(),
            VarType::Time => "Time".to_string(),
            VarType::Uint => "Unsigned Integer".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Completable for VarResult {
    async fn completion_item(
        &self,
        _ctx: crate::shared::RequestContext,
    ) -> CompletionItem {
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

#[async_trait::async_trait]
impl Completable for FunctionResult {
    async fn completion_item(
        &self,
        _ctx: crate::shared::RequestContext,
    ) -> CompletionItem {
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
