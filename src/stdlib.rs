use crate::shared::{Function, FunctionInfo};

use flux::imports;
use flux::prelude;
use flux::semantic::types::{MonoType, Record};

use std::collections::BTreeMap;
use std::iter::Iterator;

const BUILTIN_PACKAGE: &str = "builtin";

pub fn get_package_name(name: &str) -> Option<&str> {
    name.split('/').last()
}

pub fn create_function_signature(
    f: &flux::semantic::types::Function,
) -> String {
    let required = f
        .req
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| (k.clone(), format!("{}", v)))
        .collect::<Vec<_>>();

    let optional = f
        .opt
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| (k.clone(), format!("{}", v.typ)))
        .collect::<Vec<_>>();

    let pipe = match &f.pipe {
        Some(pipe) => {
            if pipe.k == "<-" {
                vec![(pipe.k.clone(), format!("{}", pipe.v))]
            } else {
                vec![(format!("<-{}", pipe.k), format!("{}", pipe.v))]
            }
        }
        None => vec![],
    };

    format!(
        "({}) -> {}",
        pipe.iter()
            .chain(required.iter().chain(optional.iter()))
            .map(|arg| format!("{}:{}", arg.0, arg.1))
            .collect::<Vec<_>>()
            .join(", "),
        format!("{}", f.retn)
    )
}

fn record_fields(
    this: &Record,
) -> impl Iterator<Item = &flux::semantic::types::Property> {
    let mut record = Some(this);
    std::iter::from_fn(move || match record {
        Some(Record::Extension { head, tail }) => {
            match tail {
                MonoType::Record(tail) => record = Some(tail),
                _ => record = None,
            }
            Some(head)
        }
        _ => None,
    })
}

fn walk_package_functions(list: &mut Vec<Function>, t: &MonoType) {
    if let MonoType::Record(record) = t {
        for head in record_fields(record) {
            if let MonoType::Fun(f) = &head.v {
                list.push(Function::new(head.k.to_string(), f));
            }
        }
    }
}

pub fn get_package_functions(name: &str) -> Vec<Function> {
    let mut list = vec![];

    if let Some(env) = imports() {
        for (key, val) in env.iter() {
            if let Some(package_name) = get_package_name(key) {
                if package_name == name {
                    walk_package_functions(
                        &mut list,
                        &val.typ().expr,
                    );
                }
            }
        }
    }

    list
}

fn walk_functions(
    package: String,
    list: &mut Vec<FunctionInfo>,
    t: &MonoType,
) {
    if let MonoType::Record(record) = t {
        for head in record_fields(record) {
            if let MonoType::Fun(f) = &head.v {
                if let Some(package_name) =
                    get_package_name(package.as_str())
                {
                    list.push(FunctionInfo::new(
                        head.k.to_string(),
                        f.as_ref(),
                        package_name.into(),
                    ));
                }
            }
        }
    }
}

pub fn get_stdlib_functions() -> Vec<FunctionInfo> {
    let mut results = vec![];

    if let Some(env) = prelude() {
        for (name, val) in env.iter() {
            if let MonoType::Fun(f) = &val.expr {
                results.push(FunctionInfo::new(
                    name.to_string(),
                    f.as_ref(),
                    BUILTIN_PACKAGE.to_string(),
                ));
            }
        }
    }

    if let Some(imports) = imports() {
        for (name, val) in imports.iter() {
            walk_functions(
                name.to_string(),
                &mut results,
                &val.typ().expr,
            );
        }
    }

    results
}

pub fn get_builtin_functions() -> Vec<Function> {
    let mut list = vec![];

    if let Some(env) = prelude() {
        for (key, val) in env.iter() {
            if let MonoType::Fun(f) = &val.expr {
                list.push(Function::new(key.to_string(), f));
            }
        }
    }

    list
}
