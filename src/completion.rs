use std::sync::Arc;

use flux::ast::walk::walk;
use flux::ast::walk::Node as AstNode;
use flux::ast::{Expression, Package, PropertyKey, SourceLocation};
use flux::parser::parse_string;
use flux::semantic::nodes::CallExpr;
use flux::semantic::nodes::Expression as SemanticExpression;
use flux::semantic::types::{
    BuiltinType, CollectionType, MonoType, Record,
};
use flux::semantic::walk::Visitor as SemanticVisitor;
use flux::{imports, prelude};
use tower_lsp::lsp_types as lsp;

use crate::shared::Function;
use crate::shared::{
    get_argument_names, get_optional_argument_names, get_package_name,
};
use crate::stdlib;
use crate::visitors::ast::NodeFinderVisitor;
use crate::visitors::semantic::{
    FunctionFinderVisitor, Import, ImportFinderVisitor,
    ObjectFunctionFinderVisitor,
};

const PRELUDE_PACKAGE: &str = "prelude";

#[derive(Clone)]
struct PackageResult {
    pub name: String,
    pub full_name: String,
}

#[derive(Clone, Debug)]
struct CompletionInfo {
    ident: String,
    position: lsp::Position,
    imports: Vec<Import>,
}

fn property_key_str(p: &PropertyKey) -> &str {
    match p {
        PropertyKey::Identifier(i) => &i.name,
        PropertyKey::StringLit(l) => &l.value,
    }
}

fn get_imports(pkg: &flux::semantic::nodes::Package) -> Vec<Import> {
    let walker = flux::semantic::walk::Node::Package(pkg);
    let mut visitor = ImportFinderVisitor::default();

    flux::semantic::walk::walk(&mut visitor, walker);
    visitor.imports
}

fn get_user_matches(
    info: CompletionInfo,
    contents: &str,
    pkg: &flux::semantic::nodes::Package,
) -> Vec<lsp::CompletionItem> {
    let completables = get_user_completables(info.position, pkg);

    let mut result: Vec<lsp::CompletionItem> = vec![];
    for x in completables {
        if x.matches(contents, &info) {
            result.push(x.completion_item(&info))
        }
    }

    result
}

fn get_identifier_matches<'a>(
    info: CompletionInfo,
    contents: &'a str,
    pkg: &'a flux::semantic::nodes::Package,
) -> impl Iterator<Item = lsp::CompletionItem> + 'a {
    get_stdlib_matches(info.ident.as_str(), &info)
        .into_iter()
        .chain(get_user_matches(info, contents, pkg))
}

fn get_trigger(params: &lsp::CompletionParams) -> Option<&str> {
    if let Some(context) = &params.context {
        context.trigger_character.as_deref()
    } else {
        None
    }
}

fn get_dot_completions(
    info: CompletionInfo,
    pkg: &flux::semantic::nodes::Package,
) -> Vec<lsp::CompletionItem> {
    let mut list = vec![];
    let name = &info.ident;
    get_specific_package_functions(
        &mut list,
        name.as_str(),
        &info.imports,
    );

    let mut items = vec![];

    for completable in get_specific_object(&info.ident, pkg) {
        items.push(completable.completion_item(&info));
    }

    for item in list {
        items.push(item.completion_item(&info));
    }

    items
}

fn new_string_arg_completion(
    value: &str,
    trigger: Option<&str>,
) -> lsp::CompletionItem {
    let insert_text = if trigger == Some("\"") {
        value.to_string()
    } else {
        format!("\"{}\"", value)
    };

    lsp::CompletionItem {
        deprecated: None,
        commit_characters: None,
        detail: None,
        label: insert_text.clone(),
        additional_text_edits: None,
        filter_text: None,
        insert_text: Some(insert_text),
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
        text_edit: None,
        kind: Some(lsp::CompletionItemKind::VALUE),
        command: None,
        data: None,
        insert_text_mode: None,
        tags: None,
    }
}

fn get_user_completables(
    pos: lsp::Position,
    pkg: &flux::semantic::nodes::Package,
) -> Vec<Arc<dyn Completable>> {
    let walker = flux::semantic::walk::Node::Package(pkg);
    let mut visitor = CompletableFinderVisitor::new(pos);

    flux::semantic::walk::walk(&mut visitor, walker);

    visitor.completables
}

fn get_stdlib_matches(
    name: &str,
    info: &CompletionInfo,
) -> Vec<lsp::CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib_completables();

    for c in completes.iter().filter(|x| x.matches(name, info)) {
        matches.push(c.completion_item(info));
    }

    matches
}

fn get_specific_package_functions(
    list: &mut Vec<Box<dyn Completable>>,
    name: &str,
    current_imports: &[Import],
) {
    if let Some(env) = imports() {
        if let Some(import) =
            current_imports.iter().find(|x| x.alias == name)
        {
            for (key, val) in env.iter() {
                if *key == import.path {
                    walk_package(key, list, &val.typ().expr);
                }
            }
        } else {
            for (key, val) in env.iter() {
                if let Some(package_name) = get_package_name(key) {
                    if package_name == name {
                        walk_package(key, list, &val.typ().expr);
                    }
                }
            }
        }
    }
}

fn get_specific_object(
    name: &str,
    pkg: &flux::semantic::nodes::Package,
) -> Vec<Arc<dyn Completable>> {
    let walker = flux::semantic::walk::Node::Package(pkg);
    let mut visitor = CompletableObjectFinderVisitor::new(name);

    flux::semantic::walk::walk(&mut visitor, walker);

    visitor.completables
}

fn get_provided_arguments(call: &flux::ast::CallExpr) -> Vec<String> {
    let mut provided = vec![];
    if let Some(Expression::Object(obj)) = call.arguments.first() {
        for prop in &obj.properties {
            match &prop.key {
                flux::ast::PropertyKey::Identifier(ident) => {
                    provided.push(ident.name.clone())
                }
                flux::ast::PropertyKey::StringLit(lit) => {
                    provided.push(lit.value.clone())
                }
            };
        }
    }

    provided
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

fn get_user_functions(
    pos: lsp::Position,
    pkg: &flux::semantic::nodes::Package,
) -> Vec<Function> {
    let walker = flux::semantic::walk::Node::Package(pkg);
    let mut visitor = FunctionFinderVisitor::new(pos);

    flux::semantic::walk::walk(&mut visitor, walker);

    visitor.functions
}

fn get_object_functions(
    object: &str,
    pkg: &flux::semantic::nodes::Package,
) -> Vec<Function> {
    let walker = flux::semantic::walk::Node::Package(pkg);
    let mut visitor = ObjectFunctionFinderVisitor::default();

    flux::semantic::walk::walk(&mut visitor, walker);

    visitor
        .results
        .into_iter()
        .filter(|obj| obj.object == object)
        .map(|obj| obj.function)
        .collect()
}

fn new_param_completion(
    name: String,
    typ: Option<&MonoType>,
    trigger: Option<&str>,
) -> lsp::CompletionItem {
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
        insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
        text_edit: None,
        kind: Some(lsp::CompletionItemKind::FIELD),
        command: None,
        data: None,
        insert_text_mode: None,
        tags: None,
    }
}

fn walk_package(
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
                    package_name: get_package_name(package)
                        .map(|s| s.to_owned()),
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

trait Completable {
    fn completion_item(
        &self,
        info: &CompletionInfo,
    ) -> lsp::CompletionItem;
    fn matches(&self, text: &str, info: &CompletionInfo) -> bool;
}

// Reports if the needle has a fuzzy match with the haystack.
//
// It is assumed that the haystack is the name of an identifier and the needle is a partial
// identifier.
fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    return haystack
        .to_lowercase()
        .contains(needle.to_lowercase().as_str());
}

impl Completable for PackageResult {
    fn completion_item(
        &self,
        info: &CompletionInfo,
    ) -> lsp::CompletionItem {
        let mut insert_text = self.name.clone();

        for import in &info.imports {
            if self.full_name == import.path {
                insert_text = import.alias.clone();
            }
        }

        lsp::CompletionItem {
            label: self.full_name.clone(),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: None,
            detail: Some("Package".to_string()),
            documentation: Some(lsp::Documentation::String(
                self.full_name.clone(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(insert_text),
            insert_text_format: Some(
                lsp::InsertTextFormat::PLAIN_TEXT,
            ),
            kind: Some(lsp::CompletionItemKind::MODULE),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: &str, _info: &CompletionInfo) -> bool {
        fuzzy_match(self.name.as_str(), text)
    }
}

impl Completable for FunctionResult {
    fn completion_item(
        &self,
        info: &CompletionInfo,
    ) -> lsp::CompletionItem {
        let imports = &info.imports;
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

    fn matches(&self, text: &str, info: &CompletionInfo) -> bool {
        if self.package == PRELUDE_PACKAGE
            && fuzzy_match(self.name.as_str(), text)
        {
            return true;
        }

        if !info.imports.iter().any(|x| self.package == x.path) {
            return false;
        }

        if let Some(mtext) = text.strip_suffix('.') {
            return info
                .imports
                .iter()
                .any(|import| import.alias == mtext);
        }

        false
    }
}

impl Completable for CompletionVarResult {
    fn completion_item(
        &self,
        _info: &CompletionInfo,
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

    fn matches(&self, text: &str, _info: &CompletionInfo) -> bool {
        fuzzy_match(self.name.as_str(), text)
    }
}

fn get_stdlib_completables() -> Vec<Box<dyn Completable>> {
    let mut list = vec![];

    get_packages(&mut list);
    get_builtins(&mut list);

    list
}

fn get_packages(list: &mut Vec<Box<dyn Completable>>) {
    if let Some(env) = imports() {
        for (key, _val) in env.iter() {
            add_package_result(key, list);
        }
    }
}

fn get_builtins(list: &mut Vec<Box<dyn Completable>>) {
    if let Some(env) = prelude() {
        for (key, val) in env.iter() {
            if key.starts_with('_') {
                // Don't allow users to "discover" private-ish functionality.
                continue;
            }
            let mut push_var_result = |var_type| {
                list.push(Box::new(VarResult {
                    name: key.to_string(),
                    package: PRELUDE_PACKAGE.to_string(),
                    package_name: None,
                    var_type,
                }));
            };
            match &val.expr {
                MonoType::Fun(f) => {
                    list.push(Box::new(FunctionResult {
                        package: PRELUDE_PACKAGE.to_string(),
                        name: key.to_string(),
                        signature: stdlib::create_function_signature(
                            f,
                        ),
                    }))
                }
                MonoType::Collection(c) => {
                    if c.collection == CollectionType::Array {
                        push_var_result(VarType::Array)
                    }
                }
                MonoType::Builtin(b) => {
                    push_var_result(VarType::from(*b))
                }
                _ => (),
            }
        }
    }
}

fn add_package_result(
    name: &str,
    list: &mut Vec<Box<dyn Completable>>,
) {
    if let Some(package_name) = get_package_name(name) {
        list.push(Box::new(PackageResult {
            name: package_name.into(),
            full_name: name.to_string(),
        }));
    }
}

struct CompletableFinderVisitor {
    pos: lsp::Position,
    completables: Vec<Arc<dyn Completable>>,
}

impl<'a> SemanticVisitor<'a> for CompletableFinderVisitor {
    fn visit(
        &mut self,
        node: flux::semantic::walk::Node<'a>,
    ) -> bool {
        let loc = node.loc();

        if defined_after(loc, self.pos) {
            return true;
        }

        match node {
            flux::semantic::walk::Node::ImportDeclaration(id) => {
                if let Some(alias) = &id.alias {
                    self.completables.push(Arc::new(
                        ImportAliasResult::new(
                            id.path.value.clone(),
                            alias.name.to_string(),
                        ),
                    ));
                }
            }

            flux::semantic::walk::Node::VariableAssgn(assgn) => {
                let name = &assgn.id.name;
                if let Some(var_type) = get_var_type(&assgn.init) {
                    self.completables.push(Arc::new(
                        CompletionVarResult {
                            var_type,
                            name: name.to_string(),
                        },
                    ));
                }

                if let Some(fun) =
                    create_function_result(name, &assgn.init)
                {
                    self.completables.push(Arc::new(fun));
                }
            }

            flux::semantic::walk::Node::OptionStmt(opt) => {
                if let flux::semantic::nodes::Assignment::Variable(
                    var_assign,
                ) = &opt.assignment
                {
                    let name = &var_assign.id.name;
                    if let Some(var_type) =
                        get_var_type(&var_assign.init)
                    {
                        self.completables.push(Arc::new(
                            CompletionVarResult {
                                name: name.to_string(),
                                var_type,
                            },
                        ));

                        return false;
                    }

                    if let Some(fun) =
                        create_function_result(name, &var_assign.init)
                    {
                        self.completables.push(Arc::new(fun));
                        return false;
                    }
                }
            }
            _ => (),
        }

        true
    }
}

impl CompletableFinderVisitor {
    fn new(pos: lsp::Position) -> Self {
        CompletableFinderVisitor {
            completables: Vec::new(),
            pos,
        }
    }
}

fn defined_after(loc: &SourceLocation, pos: lsp::Position) -> bool {
    if loc.start.line > pos.line + 1
        || (loc.start.line == pos.line + 1
            && loc.start.column > pos.character + 1)
    {
        return true;
    }

    false
}

#[derive(Clone)]
struct ImportAliasResult {
    path: String,
    alias: String,
}

impl ImportAliasResult {
    fn new(path: String, alias: String) -> Self {
        ImportAliasResult { path, alias }
    }
}

impl Completable for ImportAliasResult {
    fn completion_item(
        &self,
        _info: &CompletionInfo,
    ) -> lsp::CompletionItem {
        lsp::CompletionItem {
            label: format!("{} (self)", self.alias),
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
            insert_text_format: Some(lsp::InsertTextFormat::SNIPPET),
            kind: Some(lsp::CompletionItemKind::MODULE),
            preselect: None,
            sort_text: Some(self.alias.clone()),
            text_edit: None,

            command: None,
            data: None,
            insert_text_mode: None,
            tags: None,
        }
    }

    fn matches(&self, text: &str, _info: &CompletionInfo) -> bool {
        fuzzy_match(self.alias.as_str(), text)
    }
}

fn get_var_type(
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
                package: "self".to_string(),
                package_name: Some("self".to_string()),
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

struct CompletableObjectFinderVisitor<'a> {
    name: &'a str,
    completables: Vec<Arc<dyn Completable>>,
}

impl<'a> CompletableObjectFinderVisitor<'a> {
    fn new(name: &'a str) -> Self {
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
enum CompletionVarType {
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
    pub fn from_monotype(typ: &MonoType) -> Option<Self> {
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
    package_name: Option<String>,
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
        _info: &CompletionInfo,
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

    fn matches(&self, text: &str, info: &CompletionInfo) -> bool {
        if self.package == PRELUDE_PACKAGE
            && fuzzy_match(self.name.as_str(), text)
        {
            return true;
        }

        if !info.imports.iter().any(|x| self.package == x.path) {
            return false;
        }

        if let Some(mtext) = text.strip_suffix('.') {
            return Some(mtext.into()) == self.package_name;
        }

        false
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
pub struct UserFunctionResult {
    pub name: String,
    pub package: String,
    pub package_name: Option<String>,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
    pub signature: String,
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
        _info: &CompletionInfo,
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

    fn matches(&self, text: &str, _info: &CompletionInfo) -> bool {
        fuzzy_match(self.name.as_str(), text)
    }
}

pub fn find_completions(
    params: lsp::CompletionParams,
    contents: &str,
    sem_pkg: &flux::semantic::nodes::Package,
) -> lsp::CompletionList {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let pkg: Package = parse_string(uri.to_string(), contents).into();
    let walker = AstNode::File(&pkg.files[0]);
    let mut visitor = NodeFinderVisitor::new(position);

    walk(&mut visitor, walker);

    let mut items = Vec::new();

    if let Some(finder_node) = visitor.node {
        match finder_node.node {
            AstNode::Identifier(id) => {
                items.extend(get_identifier_matches(
                    CompletionInfo {
                        ident: id.name.clone(),
                        position,
                        imports: get_imports(sem_pkg),
                    },
                    contents,
                    sem_pkg,
                ));
            }
            AstNode::MemberExpr(member) => {
                if let Expression::Identifier(ident) = &member.object
                {
                    items = get_dot_completions(
                        CompletionInfo {
                            ident: ident.name.clone(),
                            position,
                            imports: get_imports(sem_pkg),
                        },
                        sem_pkg,
                    );
                }
            }
            AstNode::CallExpr(call) => {
                items = complete_call_expr(&params, sem_pkg, call);
            }

            AstNode::ObjectExpr(_) => {
                let parent = finder_node
                    .parent
                    .as_ref()
                    .map(|parent| &parent.node);
                if let Some(AstNode::CallExpr(call)) = parent {
                    items =
                        complete_call_expr(&params, sem_pkg, call);
                }
            }

            AstNode::StringLit(_) => {
                let parent = finder_node
                    .parent
                    .as_ref()
                    .map(|parent| &parent.node);
                if let Some(AstNode::ImportDeclaration(_)) = parent {
                    let infos = stdlib::get_package_infos();

                    let imports = get_imports(sem_pkg);

                    let mut items = vec![];
                    for info in infos {
                        if !(&imports)
                            .iter()
                            .any(|x| x.path == info.name)
                        {
                            items.push(new_string_arg_completion(
                                info.path.as_str(),
                                get_trigger(&params),
                            ));
                        }
                    }

                    return lsp::CompletionList {
                        is_incomplete: false,
                        items,
                    };
                }
            }

            _ => (),
        }
    }

    lsp::CompletionList {
        is_incomplete: false,
        items,
    }
}

fn complete_call_expr(
    params: &lsp::CompletionParams,
    sem_pkg: &flux::semantic::nodes::Package,
    call: &flux::ast::CallExpr,
) -> Vec<lsp::CompletionItem> {
    let position = params.text_document_position.position;

    let mut completion_params = Vec::new();
    let provided = get_provided_arguments(call);

    match &call.callee {
        Expression::Identifier(ident) => {
            completion_params.extend(get_function_params(
                ident.name.as_str(),
                &stdlib::get_builtin_functions(),
                &provided,
            ));

            let user_functions =
                get_user_functions(position, sem_pkg);
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

                let object_functions = get_object_functions(
                    ident.name.as_str(),
                    sem_pkg,
                );

                let key = property_key_str(&me.property);

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
            new_param_completion(name, typ.as_ref(), trigger)
        })
        .collect()
}
