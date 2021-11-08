use crate::shared::{
    get_argument_names, get_package_name, Function, FunctionInfo,
};

use flux::imports;
use flux::prelude;
use flux::semantic::types::{MonoType, Record};

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::iter::Iterator;
use std::ops::Deref;

pub const BUILTIN_PACKAGE: &str = "builtin";

#[derive(Clone)]
pub struct PackageResult {
    pub name: String,
    pub full_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub k: String,
    pub v: String,
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.k, self.v)
    }
}

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

fn get_type_string(m: MonoType, map: &mut TVarMap) -> String {
    if let MonoType::Var(t) = m {
        return map.get_letter(t);
    }
    format!("{}", m)
}

pub fn create_function_signature(
    f: flux::semantic::types::Function,
) -> String {
    let mut mapping = TVarMap::default();
    let required = f
        .req
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| Property {
            k: k.clone(),
            v: get_type_string(v.clone(), &mut mapping),
        })
        .collect::<Vec<_>>();

    let optional = f
        .opt
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| Property {
            k: String::from("?") + k,
            v: get_type_string(v.clone(), &mut mapping),
        })
        .collect::<Vec<_>>();

    let pipe = match f.pipe {
        Some(pipe) => {
            if pipe.k == "<-" {
                vec![Property {
                    k: pipe.k.clone(),
                    v: get_type_string(pipe.v, &mut mapping),
                }]
            } else {
                vec![Property {
                    k: String::from("<-") + &pipe.k,
                    v: get_type_string(pipe.v, &mut mapping),
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
        get_type_string(f.retn, &mut mapping)
    )
}

pub struct PackageInfo {
    pub name: String,
    pub path: String,
}

pub fn get_package_infos() -> Vec<PackageInfo> {
    let mut result: Vec<PackageInfo> = vec![];

    if let Some(env) = imports() {
        for (path, _val) in env.values {
            let name = get_package_name(path.clone());
            if let Some(name) = name {
                result.push(PackageInfo { name, path })
            }
        }
    }

    result
}

fn walk_package_functions(
    package: String,
    list: &mut Vec<Function>,
    t: MonoType,
) {
    if let MonoType::Record(record) = t {
        if let Record::Extension { head, tail } = record.as_ref() {
            if let MonoType::Fun(f) = &head.v {
                let mut params = vec![];

                for arg in get_argument_names(f.req.clone()) {
                    params.push(arg);
                }

                for arg in get_argument_names(f.opt.clone()) {
                    params.push(arg);
                }

                list.push(Function {
                    params,
                    name: head.k.clone(),
                });
            }

            walk_package_functions(
                package,
                list,
                tail.deref().clone(),
            );
        }
    }
}

pub fn get_package_functions(name: String) -> Vec<Function> {
    let mut list = vec![];

    if let Some(env) = imports() {
        for (key, val) in env.values {
            if let Some(package_name) = get_package_name(key.clone())
            {
                if package_name == name {
                    walk_package_functions(key, &mut list, val.expr);
                }
            }
        }
    }

    list
}

fn walk_functions(
    package: String,
    list: &mut Vec<FunctionInfo>,
    t: MonoType,
) {
    if let MonoType::Record(record) = t {
        if let Record::Extension { head, tail } = record.as_ref() {
            if let MonoType::Fun(f) = &head.v {
                if let Some(package_name) =
                    get_package_name(package.clone())
                {
                    list.push(FunctionInfo::new(
                        head.k.clone(),
                        f.as_ref(),
                        package_name,
                    ));
                }
            }

            walk_functions(package, list, tail.deref().clone());
        }
    }
}

pub fn get_stdlib_functions() -> Vec<FunctionInfo> {
    let mut results = vec![];

    if let Some(env) = prelude() {
        for (name, val) in env.values {
            if let MonoType::Fun(f) = val.expr {
                results.push(FunctionInfo::new(
                    name,
                    f.as_ref(),
                    BUILTIN_PACKAGE.to_string(),
                ));
            }
        }
    }

    if let Some(imports) = imports() {
        for (name, val) in imports.values {
            walk_functions(name, &mut results, val.expr);
        }
    }

    results
}

pub fn get_builtin_functions() -> Vec<Function> {
    let mut list = vec![];

    if let Some(env) = prelude() {
        for (key, val) in env.values {
            if let MonoType::Fun(f) = val.expr {
                let mut params = get_argument_names(f.req.clone());
                for opt in get_argument_names(f.opt.clone()) {
                    params.push(opt);
                }

                list.push(Function {
                    name: key.clone(),
                    params,
                })
            }
        }
    }

    list
}
