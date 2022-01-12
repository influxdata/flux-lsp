use std::sync::{Arc, Mutex};

use flux::analyze;
use flux::ast::walk::walk;
use flux::ast::walk::Node as AstNode;
use flux::ast::{Expression, Package, PropertyKey, SourceLocation};
use flux::parser::parse_string;
use flux::semantic::nodes::CallExpr;
use flux::semantic::nodes::Expression as SemanticExpression;
use flux::semantic::types::{BuiltinType, MonoType, Record};
use flux::semantic::walk::Visitor as SemanticVisitor;
use flux::{imports, prelude};
use lspower::lsp;

use crate::shared::get_argument_names;
use crate::shared::Function;
use crate::shared::{get_package_name, is_in_node};
use crate::stdlib;
use crate::visitors::ast::{
    CallFinderVisitor, NodeFinderVisitor, PackageInfo,
};
use crate::visitors::semantic::{
    FunctionFinderVisitor, Import, ImportFinderVisitor,
    ObjectFunctionFinderVisitor,
};

const PRELUDE_PACKAGE: &str = "prelude";

#[derive(Debug)]
pub struct Error {
    pub msg: String,
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error { msg: s }
    }
}

impl From<Error> for String {
    fn from(err: Error) -> String {
        err.msg
    }
}

pub fn find_completions(
    params: lsp::CompletionParams,
    contents: &str,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri.clone();
    let info = CompletionInfo::create(params.clone(), contents)?;

    let mut items: Vec<lsp::CompletionItem> = vec![];

    if let Some(info) = info {
        match info.completion_type {
            CompletionType::Generic => {
                let mut stdlib_matches = get_stdlib_matches(
                    info.ident.as_str(),
                    info.clone(),
                );
                items.append(&mut stdlib_matches);

                let mut user_matches =
                    get_user_matches(info, contents)?;

                items.append(&mut user_matches);
            }
            CompletionType::Bad => {}
            CompletionType::CallProperty(_func) => {
                return find_param_completions(None, params, contents)
            }
            CompletionType::Import => {
                let infos = stdlib::get_package_infos();

                let imports = get_imports_removed(
                    uri,
                    info.position,
                    contents,
                )?;

                let mut items = vec![];
                for info in infos {
                    if !(&imports).iter().any(|x| x.path == info.name)
                    {
                        items.push(new_string_arg_completion(
                            info.path.as_str(),
                            get_trigger(params.clone()),
                        ));
                    }
                }

                return Ok(lsp::CompletionList {
                    is_incomplete: false,
                    items,
                });
            }
            CompletionType::ObjectMember(_obj) => {
                return find_dot_completions(params, contents);
            }
            _ => {}
        }
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items,
    })
}

#[derive(Clone)]
enum CompletionType {
    Generic,
    Logical(flux::ast::Operator),
    CallProperty(String),
    ObjectMember(String),
    Import,
    Bad,
}

#[derive(Clone)]
struct CompletionInfo {
    completion_type: CompletionType,
    ident: String,
    position: lsp::Position,
    uri: lsp::Url,
    imports: Vec<Import>,
    package: Option<PackageInfo>,
}

impl CompletionInfo {
    fn create(
        params: lsp::CompletionParams,
        source: &str,
    ) -> Result<Option<CompletionInfo>, String> {
        let uri =
            params.text_document_position.text_document.uri.clone();
        let position = params.text_document_position.position;

        let pkg: Package =
            parse_string(uri.to_string(), source).into();
        let walker = AstNode::File(&pkg.files[0]);
        let mut visitor =
            NodeFinderVisitor::new(move_back(position, 1));

        walk(&mut visitor, walker);

        let package = PackageInfo::from(&pkg);

        let finder_node = visitor.state.node.clone();

        if let Some(finder_node) = finder_node {
            if let Some(parent) = finder_node.parent {
                match parent.node {
                    AstNode::MemberExpr(me) => {
                        if let Expression::Identifier(obj) =
                            me.object.clone()
                        {
                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::ObjectMember(
                                        obj.name.clone(),
                                    ),
                                ident: obj.name,
                                position,
                                uri: uri.clone(),
                                imports: get_imports_removed(
                                    uri, position, source,
                                )?,
                                package: Some(package),
                            }));
                        }
                    }
                    AstNode::ImportDeclaration(_id) => {
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Import,
                            ident: "".to_string(),
                            position,
                            uri: uri.clone(),
                            imports: get_imports_removed(
                                uri, position, source,
                            )?,
                            package: Some(package),
                        }));
                    }
                    AstNode::BinaryExpr(be) => {
                        match be.left.clone() {
                            Expression::Identifier(left) => {
                                let name = left.name;

                                return Ok(Some(CompletionInfo {
                                    completion_type:
                                        CompletionType::Logical(
                                            be.operator.clone(),
                                        ),
                                    ident: name,
                                    position,
                                    uri: uri.clone(),
                                    imports: get_imports(
                                        uri, position, source,
                                    )?,
                                    package: Some(package),
                                }));
                            }
                            Expression::Member(left) => {
                                if let Expression::Identifier(ident) =
                                    left.object
                                {
                                    let key = match left.property {
                                        PropertyKey::Identifier(
                                            ident,
                                        ) => ident.name,
                                        PropertyKey::StringLit(
                                            lit,
                                        ) => lit.value,
                                    };

                                    let name = format!(
                                        "{}.{}",
                                        ident.name, key
                                    );

                                    return Ok(Some(CompletionInfo {
                                                            completion_type:
                                                                CompletionType::Logical(
                                                                    be.operator.clone(),
                                                                ),
                                                                ident: name,
                                                                position,
                                                                uri: uri.clone(),
                                                                imports: get_imports(
                                                                    uri, position, source,
                                                                )?,
                            package :Some(package),
                                                        }));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }

                if let Some(grandparent) = parent.parent {
                    if let Some(greatgrandparent) = grandparent.parent
                    {
                        if let (
                            AstNode::Property(prop),
                            AstNode::ObjectExpr(_),
                            AstNode::CallExpr(call),
                        ) = (
                            parent.node,
                            grandparent.node,
                            greatgrandparent.node,
                        ) {
                            let name = match prop.key.clone() {
                                PropertyKey::Identifier(ident) => {
                                    ident.name
                                }
                                PropertyKey::StringLit(lit) => {
                                    lit.value
                                }
                            };

                            if let Expression::Identifier(func) =
                                call.callee.clone()
                            {
                                return Ok(Some(CompletionInfo {
                                    completion_type:
                                        CompletionType::CallProperty(
                                            func.name,
                                        ),
                                    ident: name,
                                    position,
                                    uri: uri.clone(),
                                    imports: get_imports(
                                        uri, position, source,
                                    )?,
                                    package: Some(package),
                                }));
                            }
                        }
                    }
                }

                match finder_node.node {
                    AstNode::BinaryExpr(be) => {
                        if let Expression::Identifier(left) =
                            be.left.clone()
                        {
                            let name = left.name;

                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Logical(
                                        be.operator.clone(),
                                    ),
                                ident: name,
                                position,
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, source,
                                )?,
                                package: Some(package),
                            }));
                        }
                    }
                    AstNode::Identifier(ident) => {
                        let name = ident.name.clone();
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Generic,
                            ident: name,
                            position,
                            uri: uri.clone(),
                            imports: get_imports(
                                uri, position, source,
                            )?,
                            package: Some(package),
                        }));
                    }
                    AstNode::BadExpr(expr) => {
                        let name = expr.text.clone();
                        return Ok(Some(CompletionInfo {
                            completion_type: CompletionType::Bad,
                            ident: name,
                            position,
                            uri: uri.clone(),
                            imports: get_imports(
                                uri, position, source,
                            )?,
                            package: Some(package),
                        }));
                    }
                    AstNode::MemberExpr(mbr) => {
                        if let Expression::Identifier(ident) =
                            &mbr.object
                        {
                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Generic,
                                ident: ident.name.clone(),
                                position,
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, source,
                                )?,
                                package: Some(package),
                            }));
                        }
                    }
                    AstNode::CallExpr(c) => {
                        if let Some(Expression::Identifier(ident)) =
                            c.arguments.last()
                        {
                            return Ok(Some(CompletionInfo {
                                completion_type:
                                    CompletionType::Generic,
                                ident: ident.name.clone(),
                                position,
                                uri: uri.clone(),
                                imports: get_imports(
                                    uri, position, source,
                                )?,
                                package: Some(package),
                            }));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(None)
    }
}

fn get_imports(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: &str,
) -> Result<Vec<Import>, String> {
    let pkg = create_completion_package(uri, pos, contents)?;
    let walker = flux::semantic::walk::Node::Package(&pkg);
    let mut visitor = ImportFinderVisitor::default();

    flux::semantic::walk::walk(&mut visitor, walker);

    Ok(visitor.imports)
}

fn get_imports_removed(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: &str,
) -> Result<Vec<Import>, String> {
    let pkg = create_completion_package_removed(uri, pos, contents)?;
    let walker = flux::semantic::walk::Node::Package(&pkg);
    let mut visitor = ImportFinderVisitor::default();

    flux::semantic::walk::walk(&mut visitor, walker);

    Ok(visitor.imports)
}

fn move_back(position: lsp::Position, count: u32) -> lsp::Position {
    lsp::Position {
        line: position.line,
        character: position.character - count,
    }
}

fn get_user_matches(
    info: CompletionInfo,
    contents: &str,
) -> Result<Vec<lsp::CompletionItem>, Error> {
    let completables = get_user_completables(
        info.uri.clone(),
        info.position,
        contents,
    )?;

    let mut result: Vec<lsp::CompletionItem> = vec![];
    for x in completables {
        if x.matches(contents, &info) {
            result.push(x.completion_item(info.clone()))
        }
    }

    Ok(result)
}

fn get_trigger(params: lsp::CompletionParams) -> Option<String> {
    if let Some(context) = params.context {
        context.trigger_character
    } else {
        None
    }
}

pub fn find_dot_completions(
    params: lsp::CompletionParams,
    contents: &str,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri.clone();
    let pos = params.text_document_position.position;
    let info = CompletionInfo::create(params, contents)?;

    if let Some(info) = info {
        let imports = info.imports.clone();

        let mut list = vec![];
        let name = info.ident.clone();
        get_specific_package_functions(
            &mut list,
            name.as_str(),
            imports,
        );

        let mut items = vec![];
        let obj_results = get_specific_object(
            info.ident.clone(),
            pos,
            uri,
            contents,
        )?;

        for completable in obj_results.into_iter() {
            items.push(completable.completion_item(info.clone()));
        }

        for item in list.into_iter() {
            items.push(item.completion_item(info.clone()));
        }

        return Ok(lsp::CompletionList {
            is_incomplete: false,
            items,
        });
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}

fn new_string_arg_completion(
    value: &str,
    trigger: Option<String>,
) -> lsp::CompletionItem {
    let trigger = trigger.unwrap_or_else(|| "".to_string());
    let insert_text = if trigger == "\"" {
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
    uri: lsp::Url,
    pos: lsp::Position,
    contents: &str,
) -> Result<Vec<Arc<dyn Completable>>, Error> {
    let pkg = create_completion_package(uri, pos, contents)?;
    let walker = flux::semantic::walk::Node::Package(&pkg);
    let mut visitor = CompletableFinderVisitor::new(pos);

    flux::semantic::walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).completables.clone());
    }

    Err(Error {
        msg: "failed to get completables".to_string(),
    })
}

fn get_stdlib_matches(
    name: &str,
    info: CompletionInfo,
) -> Vec<lsp::CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib_completables();

    for c in completes.into_iter().filter(|x| x.matches(name, &info))
    {
        matches.push(c.completion_item(info.clone()));
    }

    matches
}

pub fn find_param_completions(
    trigger: Option<String>,
    params: lsp::CompletionParams,
    source: &str,
) -> Result<lsp::CompletionList, Error> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let pkg: Package = parse_string(uri.to_string(), source).into();
    let walker = AstNode::File(&pkg.files[0]);
    let mut visitor = CallFinderVisitor::new(move_back(position, 1));

    walk(&mut visitor, walker);

    let node = visitor.state.node.clone();
    let mut items: Vec<String> = vec![];

    if let Some(node) = node {
        if let AstNode::CallExpr(call) = node {
            let provided = get_provided_arguments(call);

            if let Expression::Identifier(ident) = call.callee.clone()
            {
                items.extend(get_function_params(
                    ident.name.as_str(),
                    stdlib::get_builtin_functions(),
                    provided.clone(),
                ));

                if let Ok(user_functions) =
                    get_user_functions(uri.clone(), position, source)
                {
                    items.extend(get_function_params(
                        ident.name.as_str(),
                        user_functions,
                        provided.clone(),
                    ));
                }
            }
            if let Expression::Member(me) = call.callee.clone() {
                if let Expression::Identifier(ident) = me.object {
                    let package_functions =
                        stdlib::get_package_functions(
                            ident.name.clone(),
                        );

                    let object_functions = get_object_functions(
                        uri,
                        position,
                        ident.name.as_str(),
                        source,
                    )?;

                    let key = match me.property {
                        PropertyKey::Identifier(i) => i.name,
                        PropertyKey::StringLit(l) => l.value,
                    };

                    items.extend(get_function_params(
                        key.as_str(),
                        package_functions,
                        provided.clone(),
                    ));

                    items.extend(get_function_params(
                        key.as_str(),
                        object_functions,
                        provided,
                    ));
                }
            }
        }
    }

    Ok(lsp::CompletionList {
        is_incomplete: false,
        items: items
            .into_iter()
            .map(|x| new_param_completion(x, trigger.clone()))
            .collect(),
    })
}

fn get_specific_package_functions(
    list: &mut Vec<Box<dyn Completable>>,
    name: &str,
    current_imports: Vec<Import>,
) {
    if let Some(env) = imports() {
        if let Some(import) =
            current_imports.into_iter().find(|x| x.alias == name)
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
    name: String,
    pos: lsp::Position,
    uri: lsp::Url,
    contents: &str,
) -> Result<Vec<Arc<dyn Completable>>, Error> {
    let pkg = create_completion_package_removed(uri, pos, contents)?;
    let walker = flux::semantic::walk::Node::Package(&pkg);
    let mut visitor = CompletableObjectFinderVisitor::new(name);

    flux::semantic::walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok(state.completables.clone());
    }

    Ok(vec![])
}

fn get_provided_arguments(call: &flux::ast::CallExpr) -> Vec<String> {
    let mut provided = vec![];
    if let Some(Expression::Object(obj)) = call.arguments.first() {
        for prop in obj.properties.clone() {
            match prop.key {
                flux::ast::PropertyKey::Identifier(ident) => {
                    provided.push(ident.name)
                }
                flux::ast::PropertyKey::StringLit(lit) => {
                    provided.push(lit.value)
                }
            };
        }
    }

    provided
}

fn get_function_params(
    name: &str,
    functions: Vec<Function>,
    provided: Vec<String>,
) -> Vec<String> {
    functions.into_iter().filter(|f| f.name == name).fold(
        vec![],
        |mut acc, f| {
            acc.extend(
                f.params
                    .into_iter()
                    .filter(|p| !provided.contains(p)),
            );
            acc
        },
    )
}

fn get_user_functions(
    uri: lsp::Url,
    pos: lsp::Position,
    source: &str,
) -> Result<Vec<Function>, Error> {
    let pkg = create_completion_package(uri, pos, source)?;
    let walker = flux::semantic::walk::Node::Package(&pkg);
    let mut visitor = FunctionFinderVisitor::new(pos);

    flux::semantic::walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).functions.clone());
    }

    Err(Error {
        msg: "failed to get completables".to_string(),
    })
}

fn get_object_functions(
    uri: lsp::Url,
    pos: lsp::Position,
    object: &str,
    contents: &str,
) -> Result<Vec<Function>, Error> {
    let pkg = create_completion_package(uri, pos, contents)?;
    let walker = flux::semantic::walk::Node::Package(&pkg);
    let mut visitor = ObjectFunctionFinderVisitor::default();

    flux::semantic::walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok(state
            .results
            .clone()
            .into_iter()
            .filter(|obj| obj.object == object)
            .map(|obj| obj.function)
            .collect());
    }

    Ok(vec![])
}

fn new_param_completion(
    name: String,
    trigger: Option<String>,
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
        detail: None,
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
                    package_name: get_package_name(package),
                }));
            };

            match &head.v {
                MonoType::Fun(f) => {
                    list.push(Box::new(FunctionResult {
                        name: head.k.clone().into(),
                        package: package.to_string(),
                        signature: stdlib::create_function_signature(
                            f,
                        ),
                        required_args: get_argument_names(&f.req),
                        optional_args: get_argument_names(&f.opt),
                        package_name: get_package_name(package),
                    }));
                }
                MonoType::Arr(_) => push_var_result(
                    &head.k.clone().into(),
                    VarType::Array,
                ),
                MonoType::Builtin(b) => match b {
                    BuiltinType::Int => push_var_result(
                        &head.k.clone().into(),
                        VarType::Int,
                    ),
                    BuiltinType::Float => push_var_result(
                        &head.k.clone().into(),
                        VarType::Float,
                    ),
                    BuiltinType::Bool => push_var_result(
                        &head.k.clone().into(),
                        VarType::Bool,
                    ),
                    BuiltinType::Bytes => push_var_result(
                        &head.k.clone().into(),
                        VarType::Bytes,
                    ),
                    BuiltinType::Duration => push_var_result(
                        &head.k.clone().into(),
                        VarType::Duration,
                    ),
                    BuiltinType::Regexp => push_var_result(
                        &head.k.clone().into(),
                        VarType::Regexp,
                    ),
                    BuiltinType::String => push_var_result(
                        &head.k.clone().into(),
                        VarType::String,
                    ),
                    _ => (),
                },
                _ => (),
            }

            walk_package(package, list, tail);
        }
    }
}

trait Completable {
    fn completion_item(
        &self,
        info: CompletionInfo,
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

impl Completable for stdlib::PackageResult {
    fn completion_item(
        &self,
        info: CompletionInfo,
    ) -> lsp::CompletionItem {
        let imports = info.imports;
        let mut additional_text_edits = vec![];
        let mut insert_text = self.name.clone();

        if imports
            .iter()
            .map(|x| &x.path)
            .all(|x| *x != self.full_name)
        {
            let alias = find_alias_name(&imports, &self.name, 1);

            let new_text = if let Some(alias) = alias {
                insert_text = alias.clone();
                format!("import {} \"{}\"\n", alias, self.full_name)
            } else {
                format!("import \"{}\"\n", self.full_name)
            };

            let line = match info.package {
                Some(pi) => pi.position.line + 1,
                None => 0,
            };

            additional_text_edits.push(lsp::TextEdit {
                new_text,
                range: lsp::Range {
                    start: lsp::Position { character: 0, line },
                    end: lsp::Position { character: 0, line },
                },
            })
        } else {
            for import in imports {
                if self.full_name == import.path {
                    insert_text = import.alias;
                }
            }
        }

        lsp::CompletionItem {
            label: self.full_name.clone(),
            additional_text_edits: Some(additional_text_edits),
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
        info: CompletionInfo,
    ) -> lsp::CompletionItem {
        let imports = info.imports;
        let mut additional_text_edits = vec![];

        let contains_pkg =
            imports.into_iter().any(|x| self.package == x.path);

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
        _info: CompletionInfo,
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
                        package_name: None,
                        name: key.to_string(),
                        signature: stdlib::create_function_signature(
                            f,
                        ),
                        required_args: get_argument_names(&f.req),
                        optional_args: get_argument_names(&f.opt),
                    }))
                }
                MonoType::Arr(_) => push_var_result(VarType::Array),
                MonoType::Builtin(b) => match b {
                    BuiltinType::String => {
                        push_var_result(VarType::String)
                    }
                    BuiltinType::Int => push_var_result(VarType::Int),
                    BuiltinType::Float => {
                        push_var_result(VarType::Float)
                    }
                    BuiltinType::Bool => {
                        push_var_result(VarType::Bool)
                    }
                    BuiltinType::Bytes => {
                        push_var_result(VarType::Bytes)
                    }
                    BuiltinType::Duration => {
                        push_var_result(VarType::Duration)
                    }
                    BuiltinType::Uint => {
                        push_var_result(VarType::Uint)
                    }
                    BuiltinType::Regexp => {
                        push_var_result(VarType::Regexp)
                    }
                    BuiltinType::Time => {
                        push_var_result(VarType::Time)
                    }
                },
                _ => (),
            }
        }
    }
}

fn find_alias_name(
    imports: &[Import],
    name: &str,
    iteration: i32,
) -> Option<String> {
    let first_iteration = iteration == 1;
    let pkg_name = if first_iteration {
        name.to_string()
    } else {
        format!("{}{}", name, iteration)
    };

    for import in imports {
        if import.alias == pkg_name {
            return find_alias_name(imports, name, iteration + 1);
        }

        if let Some(initial_name) = &import.initial_name {
            if *initial_name == pkg_name && first_iteration {
                return find_alias_name(imports, name, iteration + 1);
            }
        }
    }

    if first_iteration {
        return None;
    }

    Some(format!("{}{}", name, iteration))
}

fn add_package_result(
    name: &str,
    list: &mut Vec<Box<dyn Completable>>,
) {
    let package_name = get_package_name(name);
    if let Some(package_name) = package_name {
        list.push(Box::new(stdlib::PackageResult {
            name: package_name,
            full_name: name.to_string(),
        }));
    }
}

#[derive(Default)]
struct CompletableFinderState {
    completables: Vec<Arc<dyn Completable>>,
}

struct CompletableFinderVisitor {
    pos: lsp::Position,
    state: Arc<Mutex<CompletableFinderState>>,
}

impl<'a> SemanticVisitor<'a> for CompletableFinderVisitor {
    fn visit(
        &mut self,
        node: flux::semantic::walk::Node<'a>,
    ) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let loc = node.loc();

            if defined_after(loc, self.pos) {
                return true;
            }

            if let flux::semantic::walk::Node::ImportDeclaration(id) =
                node
            {
                if let Some(alias) = id.alias.clone() {
                    (*state).completables.push(Arc::new(
                        ImportAliasResult::new(
                            id.path.value.clone(),
                            alias.name.to_string(),
                        ),
                    ));
                }
            }

            if let flux::semantic::walk::Node::VariableAssgn(assgn) =
                node
            {
                let name = assgn.id.name.clone();
                if let Some(var_type) = get_var_type(&assgn.init) {
                    (*state).completables.push(Arc::new(
                        CompletionVarResult {
                            var_type,
                            name: name.to_string(),
                        },
                    ));
                }

                if let Some(fun) = create_function_result(
                    name.to_string(),
                    &assgn.init,
                ) {
                    (*state).completables.push(Arc::new(fun));
                }
            }

            if let flux::semantic::walk::Node::OptionStmt(opt) = node
            {
                if let flux::semantic::nodes::Assignment::Variable(
                    var_assign,
                ) = &opt.assignment
                {
                    let name = var_assign.id.name.clone();
                    if let Some(var_type) =
                        get_var_type(&var_assign.init)
                    {
                        (*state).completables.push(Arc::new(
                            CompletionVarResult {
                                name: name.to_string(),
                                var_type,
                            },
                        ));

                        return false;
                    }

                    if let Some(fun) = create_function_result(
                        name.to_string(),
                        &var_assign.init,
                    ) {
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
    fn new(pos: lsp::Position) -> Self {
        CompletableFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableFinderState::default(),
            )),
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
        _info: CompletionInfo,
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
    name: String,
    expr: &SemanticExpression,
) -> Option<UserFunctionResult> {
    if let SemanticExpression::Function(f) = expr {
        if let MonoType::Fun(fun) = f.typ.clone() {
            return Some(UserFunctionResult {
                name,
                package: "self".to_string(),
                package_name: Some("self".to_string()),
                optional_args: get_argument_names(&fun.opt),
                required_args: get_argument_names(&fun.req),
                signature: stdlib::create_function_signature(&fun),
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

#[derive(Default)]
struct CompletableObjectFinderState {
    completables: Vec<Arc<dyn Completable>>,
}

struct CompletableObjectFinderVisitor {
    name: String,
    state: Arc<Mutex<CompletableObjectFinderState>>,
}

impl CompletableObjectFinderVisitor {
    fn new(name: String) -> Self {
        CompletableObjectFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableObjectFinderState::default(),
            )),
            name,
        }
    }
}

impl<'a> SemanticVisitor<'a> for CompletableObjectFinderVisitor {
    fn visit(
        &mut self,
        node: flux::semantic::walk::Node<'a>,
    ) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let name = self.name.clone();

            if let flux::semantic::walk::Node::ObjectExpr(obj) = node
            {
                if let Some(ident) = &obj.with {
                    if name == *ident.name {
                        for prop in obj.properties.clone() {
                            let name = prop.key.name;
                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    CompletionVarResult {
                                        var_type,
                                        name: name.to_string(),
                                    },
                                ));
                            }
                            if let Some(fun) = create_function_result(
                                name.to_string(),
                                &prop.value,
                            ) {
                                (*state)
                                    .completables
                                    .push(Arc::new(fun));
                            }
                        }
                    }
                }
            }

            if let flux::semantic::walk::Node::VariableAssgn(assign) =
                node
            {
                if *assign.id.name == name {
                    if let SemanticExpression::Object(obj) =
                        &assign.init
                    {
                        for prop in obj.properties.clone() {
                            let name = prop.key.name;

                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    CompletionVarResult {
                                        var_type,
                                        name: name.to_string(),
                                    },
                                ));
                            }

                            if let Some(fun) = create_function_result(
                                name.to_string(),
                                &prop.value,
                            ) {
                                (*state)
                                    .completables
                                    .push(Arc::new(fun));
                            }
                        }

                        return false;
                    }
                }
            }

            if let flux::semantic::walk::Node::OptionStmt(opt) = node
            {
                if let flux::semantic::nodes::Assignment::Variable(
                    assign,
                ) = opt.assignment.clone()
                {
                    if *assign.id.name == name {
                        if let SemanticExpression::Object(obj) =
                            assign.init
                        {
                            for prop in obj.properties.clone() {
                                let name = prop.key.name;
                                if let Some(var_type) =
                                    get_var_type(&prop.value)
                                {
                                    (*state).completables.push(
                                        Arc::new(
                                            CompletionVarResult {
                                                var_type,
                                                name: name
                                                    .to_string(),
                                            },
                                        ),
                                    );
                                }
                                if let Some(fun) =
                                    create_function_result(
                                        name.to_string(),
                                        &prop.value,
                                    )
                                {
                                    (*state)
                                        .completables
                                        .push(Arc::new(fun));
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
            MonoType::Arr(_) => CompletionVarType::Array,
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
        _info: CompletionInfo,
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
    #[allow(dead_code)]
    package_name: Option<String>,
    #[allow(dead_code)]
    required_args: Vec<String>,
    #[allow(dead_code)]
    optional_args: Vec<String>,
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

fn create_completion_package_removed(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: &str,
) -> Result<flux::semantic::nodes::Package, String> {
    let mut file = parse_string("".to_string(), contents);

    file.imports = file
        .imports
        .into_iter()
        .filter(|x| valid_node(&x.base, pos))
        .collect();

    file.body = file
        .body
        .into_iter()
        .filter(|x| valid_node(x.base(), pos))
        .collect();

    let mut pkg: Package =
        parse_string(uri.to_string(), contents).into();

    pkg.files = pkg
        .files
        .into_iter()
        .map(|curr| {
            if curr.name == uri.as_str() {
                file.clone()
            } else {
                curr
            }
        })
        .collect();

    match analyze(pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("ERROR IS HERE {}", e)),
    }
}

fn create_completion_package(
    uri: lsp::Url,
    pos: lsp::Position,
    contents: &str,
) -> Result<flux::semantic::nodes::Package, String> {
    create_filtered_package(uri, contents, |x| {
        valid_node(x.base(), pos)
    })
}

fn valid_node(
    node: &flux::ast::BaseNode,
    position: lsp::Position,
) -> bool {
    !is_in_node(position, node)
}

fn create_filtered_package<F>(
    uri: lsp::Url,
    contents: &str,
    mut filter: F,
) -> Result<flux::semantic::nodes::Package, String>
where
    F: FnMut(&flux::ast::Statement) -> bool,
{
    let mut ast_pkg: Package =
        parse_string(uri.to_string(), contents).into();

    ast_pkg.files = ast_pkg
        .files
        .into_iter()
        .map(|mut file| {
            if file.name == uri.as_str() {
                file.body = file
                    .body
                    .into_iter()
                    .filter(|x| filter(x))
                    .collect();
            }

            file
        })
        .collect();

    match analyze(ast_pkg) {
        Ok(p) => Ok(p),
        Err(e) => Err(format!("{}", e)),
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
        _info: CompletionInfo,
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
