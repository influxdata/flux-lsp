use std::sync::Arc;

use flux::ast::{Expression, PropertyKey};
use flux::semantic::nodes::CallExpr;
use flux::semantic::nodes::Expression as SemanticExpression;
use flux::semantic::types::{
    BuiltinType, CollectionType, MonoType, Record,
};
use flux::semantic::walk::Visitor as SemanticVisitor;
use lspower::lsp;

use crate::shared::Function;
use crate::shared::{
    get_argument_names, get_optional_argument_names,
};
use crate::stdlib;
use crate::visitors::semantic::{
    FunctionFinderVisitor, Import, ImportFinderVisitor,
    ObjectFunctionFinderVisitor,
};

const PRELUDE_PACKAGE: &str = "prelude";

pub fn get_imports(
    pkg: &flux::semantic::nodes::Package,
) -> Vec<Import> {
    let walker = flux::semantic::walk::Node::Package(pkg);
    let mut visitor = ImportFinderVisitor::default();

    flux::semantic::walk::walk(&mut visitor, walker);
    visitor.imports
}

fn get_function_params<'a>(
    name: &'a str,
    functions: &'a [Function],
    provided: &'a [String],
) -> impl Iterator<Item = (String, Option<MonoType>)> + 'a {
    functions.iter().filter(move |f| f.name == name).flat_map(
        move |f| {
            f.params
                .iter()
                .filter(move |(k, _)| {
                    !provided.iter().any(|p| p == k)
                })
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
        },
    )
}

pub(crate) fn walk_package(
    package: &str,
    list: &mut Vec<Box<dyn Completable>>,
    t: &MonoType,
) {
    if let MonoType::Record(record) = t {
        if let Record::Extension { head, tail } = record.as_ref() {
            let mut push_var_result = |name: &String, var_type| {
                list.push(Box::new(VarResult {
                    name: name.to_owned(),
                    var_type,
                    package: package.into(),
                }));
            };

            match &head.v {
                MonoType::Fun(f) => {
                    list.push(Box::new(FunctionResult {
                        name: head.k.clone().to_string(),
                        package: package.to_string(),
                        signature: stdlib::create_function_signature(
                            f,
                        ),
                    }));
                }
                MonoType::Collection(c) => {
                    if c.collection == CollectionType::Array {
                        push_var_result(
                            &head.k.clone().to_string(),
                            VarType::Array,
                        )
                    }
                }
                MonoType::Builtin(b) => push_var_result(
                    &head.k.clone().to_string(),
                    VarType::from(*b),
                ),
                _ => (),
            }

            walk_package(package, list, tail);
        }
    }
}

pub(crate) trait Completable {
    fn completion_item(
        &self,
        imports: &Vec<Import>,
    ) -> lsp::CompletionItem;
}

impl Completable for FunctionResult {
    fn completion_item(
        &self,
        imports: &Vec<Import>,
    ) -> lsp::CompletionItem {
        let mut additional_text_edits = vec![];

        let contains_pkg =
            imports.iter().any(|x| self.package == x.path);

        if !contains_pkg && self.package != PRELUDE_PACKAGE {
            additional_text_edits.push(lsp::TextEdit {
                new_text: format!("import \"{}\"\n", self.package),
                range: lsp::Range {
                    start: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp::Position {
                        line: 0,
                        character: 0,
                    },
                },
            })
        }

        lsp::CompletionItem {
            label: self.name.clone(),
            additional_text_edits: Some(additional_text_edits),
            commit_characters: None,
            deprecated: None,
            detail: Some(self.signature.clone()),
            documentation: None,
            filter_text: Some(self.name.clone()),
            insert_text: None,
            insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
            kind: Some(lsp::CompletionItemKind::FUNCTION),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }
}

impl Completable for CompletionVarResult {
    fn completion_item(
        &self,
        _imports: &Vec<Import>,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} (self)", self.name),
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
                lsp::InsertTextFormat::PLAIN_TEXT,
            ),
            kind: Some(lsp::CompletionItemKind::VARIABLE),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }
}

pub fn get_var_type(
    expr: &SemanticExpression,
) -> Option<CompletionVarType> {
    if let Some(typ) =
        CompletionVarType::from_monotype(&expr.type_of())
    {
        return Some(typ);
    }

    match expr {
        SemanticExpression::Object(_) => {
            Some(CompletionVarType::Object)
        }
        SemanticExpression::Call(c) => {
            let result_type = follow_function_pipes(c);

            match CompletionVarType::from_monotype(result_type) {
                Some(typ) => Some(typ),
                None => match result_type {
                    MonoType::Record(_) => {
                        Some(CompletionVarType::Record)
                    }
                    _ => None,
                },
            }
        }
        _ => None,
    }
}

fn create_function_result(
    name: &str,
    expr: &SemanticExpression,
) -> Option<UserFunctionResult> {
    if let SemanticExpression::Function(f) = expr {
        if let MonoType::Fun(fun) = &f.typ {
            return Some(UserFunctionResult {
                name: name.into(),
                optional_args: get_optional_argument_names(&fun.opt),
                required_args: get_argument_names(&fun.req),
                signature: stdlib::create_function_signature(fun),
            });
        }
    }

    None
}

fn follow_function_pipes(c: &CallExpr) -> &MonoType {
    if let Some(SemanticExpression::Call(call)) = &c.pipe {
        return follow_function_pipes(call);
    }

    &c.typ
}

pub(crate) struct CompletableObjectFinderVisitor<'a> {
    name: &'a str,
    pub completables: Vec<Arc<dyn Completable>>,
}

impl<'a> CompletableObjectFinderVisitor<'a> {
    pub fn new(name: &'a str) -> Self {
        CompletableObjectFinderVisitor {
            completables: Vec::new(),
            name,
        }
    }
}

impl<'a> SemanticVisitor<'a> for CompletableObjectFinderVisitor<'_> {
    fn visit(
        &mut self,
        node: flux::semantic::walk::Node<'a>,
    ) -> bool {
        let name = self.name;

        match node {
            flux::semantic::walk::Node::ObjectExpr(obj) => {
                if let Some(ident) = &obj.with {
                    if ident.name == name {
                        for prop in &obj.properties {
                            let name = &prop.key.name;
                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                self.completables.push(Arc::new(
                                    CompletionVarResult {
                                        var_type,
                                        name: name.to_string(),
                                    },
                                ));
                            }
                            if let Some(fun) = create_function_result(
                                name,
                                &prop.value,
                            ) {
                                self.completables.push(Arc::new(fun));
                            }
                        }
                    }
                }
            }

            flux::semantic::walk::Node::VariableAssgn(assign) => {
                if assign.id.name == name {
                    if let SemanticExpression::Object(obj) =
                        &assign.init
                    {
                        for prop in &obj.properties {
                            let name = &prop.key.name;

                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                self.completables.push(Arc::new(
                                    CompletionVarResult {
                                        var_type,
                                        name: name.to_string(),
                                    },
                                ));
                            }

                            if let Some(fun) = create_function_result(
                                name,
                                &prop.value,
                            ) {
                                self.completables.push(Arc::new(fun));
                            }
                        }

                        return false;
                    }
                }
            }

            flux::semantic::walk::Node::OptionStmt(opt) => {
                if let flux::semantic::nodes::Assignment::Variable(
                    assign,
                ) = &opt.assignment
                {
                    if assign.id.name == name {
                        if let SemanticExpression::Object(obj) =
                            &assign.init
                        {
                            for prop in &obj.properties {
                                let name = &prop.key.name;
                                if let Some(var_type) =
                                    get_var_type(&prop.value)
                                {
                                    self.completables.push(Arc::new(
                                        CompletionVarResult {
                                            var_type,
                                            name: name.to_string(),
                                        },
                                    ));
                                }
                                if let Some(fun) =
                                    create_function_result(
                                        name,
                                        &prop.value,
                                    )
                                {
                                    self.completables
                                        .push(Arc::new(fun));
                                }
                            }
                            return false;
                        }
                    }
                }
            }
            _ => (),
        }

        true
    }
}

#[derive(Clone)]
struct CompletionVarResult {
    name: String,
    var_type: CompletionVarType,
}

#[derive(Clone)]
pub enum CompletionVarType {
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

impl CompletionVarType {
    fn from_monotype(typ: &MonoType) -> Option<Self> {
        Some(match typ {
            MonoType::Collection(c) => match c.collection {
                CollectionType::Array => CompletionVarType::Array,
                _ => return None,
            },
            MonoType::Builtin(b) => match b {
                BuiltinType::Duration => CompletionVarType::Duration,
                BuiltinType::Int => CompletionVarType::Int,
                BuiltinType::Bool => CompletionVarType::Bool,
                BuiltinType::Float => CompletionVarType::Float,
                BuiltinType::String => CompletionVarType::String,
                BuiltinType::Regexp => CompletionVarType::Regexp,
                BuiltinType::Uint => CompletionVarType::Uint,
                BuiltinType::Time => CompletionVarType::Time,
                _ => return None,
            },
            _ => return None,
        })
    }
}

#[derive(Clone)]
struct VarResult {
    name: String,
    var_type: VarType,
    package: String,
}

impl VarResult {
    fn detail(&self) -> String {
        match self.var_type {
            VarType::Array => "Array".to_string(),
            VarType::Bool => "Boolean".to_string(),
            VarType::Bytes => "Bytes".to_string(),
            VarType::Duration => "Duration".to_string(),
            VarType::Float => "Float".to_string(),
            VarType::Int => "Integer".to_string(),
            VarType::Regexp => "Regular Expression".to_string(),
            VarType::String => "String".to_string(),
            VarType::Uint => "Uint".to_string(),
            VarType::Time => "Time".to_string(),
        }
    }
}

impl Completable for VarResult {
    fn completion_item(
        &self,
        _imports: &Vec<Import>,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} ({})", self.name, self.package),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some(self.detail()),
            documentation: Some(lsp::Documentation::String(format!(
                "from {}",
                self.package
            ))),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: Some(
                lsp::InsertTextFormat::PLAIN_TEXT,
            ),
            kind: Some(lsp::CompletionItemKind::VARIABLE),
            preselect: None,
            sort_text: Some(format!(
                "{} {}",
                self.name, self.package
            )),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }
}

impl CompletionVarResult {
    fn detail(&self) -> String {
        match self.var_type {
            CompletionVarType::Array => "Array".to_string(),
            CompletionVarType::Bool => "Boolean".to_string(),
            CompletionVarType::Duration => "Duration".to_string(),
            CompletionVarType::Float => "Float".to_string(),
            CompletionVarType::Int => "Integer".to_string(),
            CompletionVarType::Object => "Object".to_string(),
            CompletionVarType::Regexp => {
                "Regular Expression".to_string()
            }
            CompletionVarType::String => "String".to_string(),
            CompletionVarType::Record => "Record".to_string(),
            CompletionVarType::Time => "Time".to_string(),
            CompletionVarType::Uint => "Unsigned Integer".to_string(),
        }
    }
}

#[derive(Clone)]
struct FunctionResult {
    name: String,
    package: String,
    signature: String,
}
#[derive(Clone)]
enum VarType {
    Int,
    String,
    Array,
    Float,
    Bool,
    Bytes,
    Duration,
    Regexp,
    Uint,
    Time,
}

impl From<BuiltinType> for VarType {
    fn from(b: BuiltinType) -> Self {
        match b {
            BuiltinType::String => VarType::String,
            BuiltinType::Int => VarType::Int,
            BuiltinType::Float => VarType::Float,
            BuiltinType::Bool => VarType::Bool,
            BuiltinType::Bytes => VarType::Bytes,
            BuiltinType::Duration => VarType::Duration,
            BuiltinType::Uint => VarType::Uint,
            BuiltinType::Regexp => VarType::Regexp,
            BuiltinType::Time => VarType::Time,
        }
    }
}

#[derive(Clone)]
struct UserFunctionResult {
    name: String,
    required_args: Vec<String>,
    optional_args: Vec<String>,
    signature: String,
}

impl UserFunctionResult {
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

impl Completable for UserFunctionResult {
    fn completion_item(
        &self,
        _imports: &Vec<Import>,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} (self)", self.name),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some(self.signature.clone()),
            documentation: Some(lsp::Documentation::String(
                "from self".to_string(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.insert_text()),
            insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
            kind: Some(lsp::CompletionItemKind::FUNCTION),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }
}

pub fn complete_call_expr(
    params: &lsp::CompletionParams,
    sem_pkg: &flux::semantic::nodes::Package,
    call: &flux::ast::CallExpr,
) -> Vec<lsp::CompletionItem> {
    let position = params.text_document_position.position;

    let mut completion_params = Vec::new();
    let provided = if let Some(Expression::Object(obj)) =
        call.arguments.first()
    {
        obj.properties
            .iter()
            .map(|prop| match &prop.key {
                flux::ast::PropertyKey::Identifier(identifier) => {
                    identifier.name.clone()
                }
                flux::ast::PropertyKey::StringLit(literal) => {
                    literal.value.clone()
                }
            })
            .collect()
    } else {
        vec![]
    };

    match &call.callee {
        Expression::Identifier(ident) => {
            completion_params.extend(get_function_params(
                ident.name.as_str(),
                &stdlib::get_builtin_functions(),
                &provided,
            ));

            let user_functions = {
                let walker =
                    flux::semantic::walk::Node::Package(sem_pkg);
                let mut visitor =
                    FunctionFinderVisitor::new(position);

                flux::semantic::walk::walk(&mut visitor, walker);

                visitor.functions
            };
            completion_params.extend(get_function_params(
                ident.name.as_str(),
                &user_functions,
                &provided,
            ));
        }
        Expression::Member(me) => {
            if let Expression::Identifier(ident) = &me.object {
                let package_functions =
                    stdlib::get_package_functions(&ident.name);

                let object_functions: Vec<Function> = {
                    let walker =
                        flux::semantic::walk::Node::Package(sem_pkg);
                    let mut visitor =
                        ObjectFunctionFinderVisitor::default();

                    flux::semantic::walk::walk(&mut visitor, walker);

                    visitor
                        .results
                        .into_iter()
                        .filter(|obj| {
                            obj.object == ident.name.as_str()
                        })
                        .map(|obj| obj.function)
                        .collect()
                };

                let key = match &me.property {
                    PropertyKey::Identifier(i) => &i.name,
                    PropertyKey::StringLit(l) => &l.value,
                };

                completion_params.extend(get_function_params(
                    key,
                    &package_functions,
                    &provided,
                ));

                completion_params.extend(get_function_params(
                    key,
                    &object_functions,
                    &provided,
                ));
            }
        }
        _ => (),
    }

    let trigger = params
        .context
        .as_ref()
        .and_then(|context| context.trigger_character.as_deref());

    completion_params
        .into_iter()
        .map(|(name, typ)| {
            let insert_text = if let Some(trigger) = trigger {
                if trigger == "(" {
                    format!("{}: ", name)
                } else {
                    format!(" {}: ", name)
                }
            } else {
                format!("{}: ", name)
            };

            lsp::CompletionItem {
                deprecated: None,
                commit_characters: None,
                detail: typ.map(|typ| typ.to_string()),
                label: name,
                additional_text_edits: None,
                filter_text: None,
                insert_text: Some(insert_text),
                documentation: None,
                sort_text: None,
                preselect: None,
                insert_text_format: Some(
                    lsp::InsertTextFormat::SNIPPET,
                ),
                text_edit: None,
                kind: Some(lsp::CompletionItemKind::FIELD),
                command: None,
                data: None,
                insert_text_mode: None,
                tags: None,
            }
        })
        .collect()
}
