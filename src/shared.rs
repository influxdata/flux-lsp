use lspower::lsp;

use flux::semantic::types::MonoType;

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
