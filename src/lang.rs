/// Tools for working with the Flux language and APIs for bridging
/// the gap between Flux language data structures and the needs of the LSP.
use flux::prelude;
use flux::semantic::types::{MonoType, Record};
use lazy_static;
use lspower::lsp;

use std::collections::BTreeMap;
use std::iter::Iterator;

const BUILTIN_PACKAGE: &str = "builtin";
lazy_static::lazy_static! {
    pub static ref STDLIB: flux::semantic::import::Packages = flux::imports().expect("Could not initialize stdlib.");
}

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
        f.retn
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

    for (key, val) in STDLIB.iter() {
        if let Some(package_name) = get_package_name(key) {
            if package_name == name {
                walk_package_functions(&mut list, &val.typ().expr);
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

    for (name, val) in STDLIB.iter() {
        walk_functions(
            name.to_string(),
            &mut results,
            &val.typ().expr,
        );
    }

    results
}

pub fn get_builtin_functions() -> Vec<Function> {
    if let Some(env) = prelude() {
        env.iter()
            .filter(|(_key, val)| {
                matches!(&val.expr, MonoType::Fun(_))
            })
            .map(|(key, val)| match &val.expr {
                MonoType::Fun(f) => Function::new(key.into(), f),
                _ => unreachable!(
                    "Previous filter call failed. Got: {}",
                    val.expr
                ),
            })
            .collect()
    } else {
        vec![]
    }
}

pub struct FunctionInfo {
    pub name: String,
    pub package_name: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

impl FunctionInfo {
    pub fn new(
        name: String,
        f: &flux::semantic::types::Function,
        package_name: String,
    ) -> Self {
        FunctionInfo {
            name,
            package_name,
            required_args: get_argument_names(&f.req),
            optional_args: get_optional_argument_names(&f.opt),
        }
    }

    pub fn signatures(&self) -> Vec<FunctionSignature> {
        let mut result = vec![FunctionSignature {
            name: self.name.clone(),
            arguments: self.required_args.clone(),
        }];

        let mut combos = vec![];
        let length = self.optional_args.len();
        for i in 1..length {
            let c: Vec<Vec<String>> =
                combinations::Combinations::new(
                    self.optional_args.clone(),
                    i,
                )
                .collect();
            combos.extend(c);
        }
        combos.push(self.optional_args.clone());

        for l in combos {
            let mut arguments = self.required_args.clone();
            arguments.extend(l.clone());

            result.push(FunctionSignature {
                name: self.name.clone(),
                arguments,
            });
        }

        result
    }
}

pub struct FunctionSignature {
    pub name: String,
    pub arguments: Vec<String>,
}

impl FunctionSignature {
    pub fn create_signature(&self) -> String {
        let args: String = self
            .arguments
            .iter()
            .map(|x| format!("{}: ${}", x, x))
            .collect::<Vec<String>>()
            .join(" , ");

        let result = format!("{}({})", self.name, args);

        result
    }

    pub fn create_parameters(
        &self,
    ) -> Vec<lsp::ParameterInformation> {
        self.arguments
            .iter()
            .map(|x| lsp::ParameterInformation {
                label: lsp::ParameterLabel::Simple(format!("${}", x)),
                documentation: None,
            })
            .collect()
    }
}

#[allow(clippy::implicit_hasher)]
pub fn get_argument_names(
    args: &std::collections::BTreeMap<String, MonoType>,
) -> Vec<String> {
    args.keys().map(String::from).collect()
}

#[allow(clippy::implicit_hasher)]
pub fn get_optional_argument_names(
    args: &std::collections::BTreeMap<
        String,
        flux::semantic::types::Argument<MonoType>,
    >,
) -> Vec<String> {
    args.keys().map(String::from).collect()
}

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Option<MonoType>)>,
}

impl Function {
    pub(crate) fn new(
        name: String,
        f: &flux::semantic::types::Function,
    ) -> Self {
        let params = f
            .req
            .iter()
            .chain(f.opt.iter().map(|p| (p.0, &p.1.typ)))
            .chain(f.pipe.as_ref().map(|p| (&p.k, &p.v)))
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();
        Self { name, params }
    }

    pub(crate) fn from_expr(
        name: String,
        expr: &flux::semantic::nodes::FunctionExpr,
    ) -> Self {
        let params = expr
            .params
            .iter()
            .map(|p| {
                (
                    p.key.name.to_string(),
                    expr.typ.parameter(&p.key.name).cloned(),
                )
            })
            .collect::<Vec<_>>();
        Self { name, params }
    }
}
