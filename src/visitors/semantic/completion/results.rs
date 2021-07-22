use crate::shared::signatures::get_argument_names;
use crate::shared::{CompletionInfo, RequestContext};
use crate::stdlib::{create_function_signature, Completable};
use crate::visitors::semantic::completion::utils::follow_function_pipes;

use flux::semantic::nodes::*;
use flux::semantic::types::MonoType;

use lsp_types as lsp;

#[derive(Clone)]
pub struct ImportAliasResult {
    pub path: String,
    pub alias: String,
}

impl ImportAliasResult {
    pub fn new(path: String, alias: String) -> Self {
        ImportAliasResult { path, alias }
    }
}

#[async_trait::async_trait]
impl Completable for ImportAliasResult {
    async fn completion_item(
        &self,
        _ctx: RequestContext,
        _info: CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} ({})", self.alias, "self".to_string()),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some("Package".to_string()),
            documentation: Some(lsp::Documentation::String(format!(
                "from {}",
                self.path
            ))),
            filter_text: Some(self.alias.clone()),
            insert_text: Some(self.alias.clone()),
            insert_text_format: Some(lsp::InsertTextFormat::Snippet),
            kind: Some(lsp::CompletionItemKind::Module),
            preselect: None,
            sort_text: Some(self.alias.clone()),
            text_edit: None,

            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, _text: String, _info: CompletionInfo) -> bool {
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
        _ctx: RequestContext,
        _info: CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} ({})", self.name, "self".to_string()),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some(self.signature.clone()),
            documentation: Some(lsp::Documentation::String(
                "from self".to_string(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.insert_text()),
            insert_text_format: Some(lsp::InsertTextFormat::Snippet),
            kind: Some(lsp::CompletionItemKind::Function),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, _text: String, _info: CompletionInfo) -> bool {
        true
    }
}

#[derive(Clone)]
pub enum VarType {
    Int,
    String,
    Array,
    Float,
    Bool,
    Duration,
    Object,
    Regexp,
    Record,
    Uint,
    Time,
}

#[derive(Clone)]
pub struct VarResult {
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
            VarType::Record => "Record".to_string(),
            VarType::Time => "Time".to_string(),
            VarType::Uint => "Unsigned Integer".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Completable for VarResult {
    async fn completion_item(
        &self,
        _ctx: RequestContext,
        _info: CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} ({})", self.name, "self".to_string()),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some(self.detail()),
            documentation: Some(lsp::Documentation::String(
                "from self".to_string(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: Some(
                lsp::InsertTextFormat::PlainText,
            ),
            kind: Some(lsp::CompletionItemKind::Variable),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, _text: String, _info: CompletionInfo) -> bool {
        true
    }
}

pub fn get_var_type(expr: &Expression) -> Option<VarType> {
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
            let result_type = follow_function_pipes(c);

            match result_type {
                MonoType::Int => Some(VarType::Int),
                MonoType::Float => Some(VarType::Float),
                MonoType::Bool => Some(VarType::Bool),
                MonoType::Arr(_) => Some(VarType::Array),
                MonoType::Duration => Some(VarType::Duration),
                MonoType::Record(_) => Some(VarType::Record),
                MonoType::String => Some(VarType::String),
                MonoType::Uint => Some(VarType::Uint),
                MonoType::Time => Some(VarType::Time),
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn create_function_result(
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
