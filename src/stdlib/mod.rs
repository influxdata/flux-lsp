use crate::protocol::responses::{
    CompletionItem, CompletionItemKind, InsertTextFormat,
};

use flux::semantic::types::MonoType;
use flux::semantic::types::Row;
use libstd::{imports, prelude};

use std::collections::BTreeMap;
use std::iter::Iterator;

fn contains(l: Vec<String>, m: String) -> bool {
    l.into_iter().find(|x| x.as_str() == m.as_str()) != None
}

pub trait Completable {
    fn completion_item(&self) -> CompletionItem;
    fn matches(&self, text: String, imports: Vec<String>) -> bool;
}

#[derive(Clone)]
pub enum VarType {
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

#[derive(Clone)]
pub struct VarResult {
    pub name: String,
    pub var_type: VarType,
    pub package: String,
    pub package_name: Option<String>,
}

impl VarResult {
    pub fn detail(&self) -> String {
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
    fn completion_item(&self) -> CompletionItem {
        CompletionItem {
            label: format!("{} ({})", self.name, self.package),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some(self.detail()),
            documentation: Some(format!("from {}", self.package)),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: InsertTextFormat::PlainText,
            kind: Some(CompletionItemKind::Variable),
            preselect: None,
            sort_text: Some(format!(
                "{} {}",
                self.name, self.package
            )),
            text_edit: None,
        }
    }

    fn matches(&self, text: String, imports: Vec<String>) -> bool {
        if self.package == "builtin" && !text.ends_with('.') {
            return true;
        }

        if !contains(imports, self.package.clone()) {
            return false;
        }

        if text.ends_with('.') {
            let mtext = text[..text.len() - 1].to_string();
            return Some(mtext) == self.package_name;
        }

        false
    }
}

#[derive(Clone)]
pub struct PackageResult {
    pub name: String,
    pub full_name: String,
}

impl Completable for PackageResult {
    fn completion_item(&self) -> CompletionItem {
        CompletionItem {
            label: self.name.clone(),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some("Package".to_string()),
            documentation: Some(self.full_name.clone()),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: InsertTextFormat::PlainText,
            kind: Some(CompletionItemKind::Module),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
        }
    }

    fn matches(&self, text: String, imports: Vec<String>) -> bool {
        if !contains(imports, self.full_name.clone()) {
            return false;
        }
        if !text.ends_with('.') {
            let name = self.name.to_lowercase();
            let mtext = text.to_lowercase();
            return name.starts_with(mtext.as_str());
        }

        false
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
            label: self.name.clone(),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some(self.signature.clone()),
            documentation: Some(format!("from {}", self.package)),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.insert_text()),
            insert_text_format: InsertTextFormat::Snippet,
            kind: Some(CompletionItemKind::Function),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
        }
    }

    fn matches(&self, text: String, imports: Vec<String>) -> bool {
        if self.package == "builtin" && !text.ends_with('.') {
            return true;
        }

        if !contains(imports, self.package.clone()) {
            return false;
        }

        if text.ends_with('.') {
            let mtext = text[..text.len() - 1].to_string();
            return Some(mtext) == self.package_name;
        }

        false
    }
}

fn create_function_signature(
    f: flux::semantic::types::Function,
) -> String {
    let required = f
        .req
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| flux::semantic::types::Property {
            k: k.clone(),
            v: v.clone(),
        })
        .collect::<Vec<_>>();

    let optional = f
        .opt
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| flux::semantic::types::Property {
            k: String::from("?") + &k,
            v: v.clone(),
        })
        .collect::<Vec<_>>();

    let pipe = match f.pipe {
        Some(pipe) => {
            if pipe.k == "<-" {
                vec![pipe.clone()]
            } else {
                vec![flux::semantic::types::Property {
                    k: String::from("<-") + &pipe.k,
                    v: pipe.v.clone(),
                }]
            }
        }
        None => vec![],
    };

    format!(
        "({}) -> {}",
        pipe.iter()
            .chain(required.iter().chain(optional.iter()))
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        f.retn
    )
}

fn walk(
    package: String,
    list: &mut Vec<Box<dyn Completable>>,
    t: MonoType,
) {
    if let MonoType::Row(row) = t {
        if let Row::Extension { head, tail } = *row {
            match head.v {
                MonoType::Fun(f) => {
                    list.push(Box::new(FunctionResult {
                        name: head.k,
                        package: package.clone(),
                        signature: create_function_signature(
                            (*f).clone(),
                        ),
                        required_args: f
                            .req
                            .keys()
                            .map(String::from)
                            .collect(),
                        optional_args: f
                            .opt
                            .keys()
                            .map(String::from)
                            .collect(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Int => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Int,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Float => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Float,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Bool => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Bool,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Arr(_) => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Array,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Bytes => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Bytes,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Duration => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Duration,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Regexp => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Regexp,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::String => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::String,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                _ => {}
            }

            walk(package, list, tail);
        }
    }
}

pub fn get_package_name(name: String) -> Option<String> {
    let items = name.split('/');

    if let Some(n) = items.last() {
        Some(n.to_string())
    } else {
        None
    }
}

pub fn add_package_result(
    name: String,
    list: &mut Vec<Box<dyn Completable>>,
) {
    let package_name = get_package_name(name.clone());
    if let Some(package_name) = package_name {
        list.push(Box::new(PackageResult {
            name: package_name,
            full_name: name.clone(),
        }));
    }
}

fn get_imports(list: &mut Vec<Box<dyn Completable>>) {
    let env = imports().unwrap();

    for (key, val) in env.values {
        add_package_result(key.clone(), list);
        walk(key, list, val.expr);
    }
}

pub fn get_builtins(list: &mut Vec<Box<dyn Completable>>) {
    let env = prelude().unwrap();

    for (key, val) in env.values {
        match val.expr {
            MonoType::Fun(f) => list.push(Box::new(FunctionResult {
                package: "builtin".to_string(),
                package_name: None,
                name: key.clone(),
                signature: create_function_signature((*f).clone()),
                required_args: f
                    .req
                    .keys()
                    .map(String::from)
                    .collect(),
                optional_args: f
                    .opt
                    .keys()
                    .map(String::from)
                    .collect(),
            })),
            MonoType::String => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::String,
            })),
            MonoType::Int => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Int,
            })),
            MonoType::Float => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Float,
            })),
            MonoType::Arr(_) => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Array,
            })),
            MonoType::Bool => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Bool,
            })),
            MonoType::Bytes => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Bytes,
            })),
            MonoType::Duration => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Duration,
            })),
            MonoType::Uint => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Uint,
            })),
            MonoType::Regexp => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Regexp,
            })),
            MonoType::Time => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Time,
            })),
            _ => {}
        }
    }
}

pub fn get_stdlib() -> Vec<Box<dyn Completable>> {
    let mut list = vec![];

    get_imports(&mut list);
    get_builtins(&mut list);

    list
}
