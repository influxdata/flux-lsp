use std::collections::BTreeMap;

use flux::semantic::types::{Function, MonoType};

use crate::shared::all_combos;

use lspower::lsp;

#[allow(clippy::implicit_hasher)]
pub fn get_argument_names(
    args: BTreeMap<String, MonoType>,
) -> Vec<String> {
    args.keys().map(String::from).collect()
}

pub struct FunctionSignature {
    pub name: String,
    pub arguments: Vec<String>,
}

impl FunctionSignature {
    pub fn create_signature(&self) -> String {
        let args: String = self
            .arguments
            .clone()
            .into_iter()
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
            .clone()
            .into_iter()
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
        f: &Function,
        package_name: String,
    ) -> Self {
        FunctionInfo {
            name,
            package_name,
            required_args: get_argument_names(f.req.clone()),
            optional_args: get_argument_names(f.opt.clone()),
        }
    }

    pub fn signatures(&self) -> Vec<FunctionSignature> {
        let mut result = vec![FunctionSignature {
            name: self.name.clone(),
            arguments: self.required_args.clone(),
        }];

        for l in all_combos(self.optional_args.clone()) {
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
