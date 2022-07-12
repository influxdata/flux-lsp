use crate::shared::{get_package_name, Function, FunctionInfo};

use flux::imports;
use flux::prelude;
use flux::semantic::types::{MonoType, Record};

use std::collections::{BTreeMap, HashMap};
use std::iter::Iterator;

const BUILTIN_PACKAGE: &str = "builtin";

struct TVarMap {
    pub mapping: HashMap<flux::semantic::types::Tvar, char>,
    pub current_letter: char,
}

impl TVarMap {
    pub fn default() -> Self {
        TVarMap {
            mapping: HashMap::new(),
            current_letter: 'A',
        }
    }

    fn increment(&mut self) {
        let c = std::char::from_u32(self.current_letter as u32 + 1)
            .unwrap_or(self.current_letter);
        self.current_letter = c
    }

    fn add(&mut self, v: flux::semantic::types::Tvar) -> String {
        let c = self.current_letter;
        self.increment();
        self.mapping.insert(v, c);

        format!("{}", c)
    }

    pub fn get_letter(
        &mut self,
        v: flux::semantic::types::Tvar,
    ) -> String {
        if let Some(result) = self.mapping.get(&v) {
            format!("{}", *result)
        } else {
            self.add(v)
        }
    }
}

fn get_type_string(m: &MonoType, map: &mut TVarMap) -> String {
    if let MonoType::Var(t) = m {
        return map.get_letter(*t);
    }
    format!("{}", m)
}

pub fn create_function_signature(
    f: &flux::semantic::types::Function,
) -> String {
    let mut mapping = TVarMap::default();
    let required = f
        .req
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| (k.clone(), get_type_string(v, &mut mapping)))
        .collect::<Vec<_>>();

    let optional = f
        .opt
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| (k.clone(), get_type_string(&v.typ, &mut mapping)))
        .collect::<Vec<_>>();

    let pipe = match &f.pipe {
        Some(pipe) => {
            if pipe.k == "<-" {
                vec![(pipe.k.clone(), get_type_string(&pipe.v, &mut mapping))]
            } else {
                vec![(format!("<-{}", pipe.k), get_type_string(&pipe.v, &mut mapping))]
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
        get_type_string(&f.retn, &mut mapping)
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
