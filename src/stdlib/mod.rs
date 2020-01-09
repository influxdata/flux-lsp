use crate::protocol::responses::{CompletionItem, InsertTextFormat, CompletionItemKind};

use flux::semantic::types::MonoType;
use flux::semantic::types::Row;
use libstd::imports;

use std::iter::Iterator;

pub trait Completable {
  fn completion_item(&self) -> CompletionItem;
  fn matches(&self, text: String) -> bool;
}

#[derive(Clone)]
pub struct VarResult {
  pub name: String,
  pub package: String
}

impl Completable for VarResult {
  fn completion_item(&self) -> CompletionItem {
      CompletionItem {
          label: format!("{} ({})", self.name, self.package),
          additional_text_edits: None,
          commit_characters: None,
          deprecated: false,
          detail: Some(format!("package: {}", self.package)),
          documentation: Some(format!("package: {}", self.package)),
          filter_text: Some(self.name.clone()),
          insert_text: Some(self.name.clone()),
          insert_text_format: InsertTextFormat::PlainText,
          kind: Some(CompletionItemKind::Variable),
          preselect: None,
          sort_text: Some(format!("{} {}", self.name, self.package)),
          text_edit: None,
      }
  }

  fn matches(&self, text: String) -> bool {
    let name = self.name.to_lowercase();
    let mtext = text.to_lowercase();

    name.starts_with(mtext.as_str())
  }
}

#[derive(Clone)]
pub struct FunctionResult {
    pub name: String,
    pub package: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

impl FunctionResult {
  fn insert_text(&self) -> String {
      let mut insert_text = format!("{}(", self.name);

      for (index, arg) in self.required_args.iter().enumerate() {
        insert_text += (format!("{}: ${}", arg, index+1)).as_str();

        if index != self.required_args.len() - 1 {
          insert_text += ", ";
        }
      }

      insert_text += ")$0";

      insert_text
  }
}

impl Completable for FunctionResult {
  fn completion_item(&self) -> CompletionItem {
      CompletionItem {
          label: format!("{} ({})", self.name, self.package),
          additional_text_edits: None,
          commit_characters: None,
          deprecated: false,
          detail: Some(format!("package: {}", self.package)),
          documentation: Some(format!("package: {}", self.package)),
          filter_text: Some(self.name.clone()),
          insert_text: Some(self.insert_text()),
          insert_text_format: InsertTextFormat::Snippet,
          kind: Some(CompletionItemKind::Function),
          preselect: None,
          sort_text: Some(format!("{} {}", self.name, self.package)),
          text_edit: None,
      }
  }

  fn matches(&self, text: String) -> bool {
    let name = self.name.to_lowercase();
    let mtext = text.to_lowercase();

    name.starts_with(mtext.as_str())
  }
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
                      required_args: f.req.keys().map(String::from).collect(),
                      optional_args: f.opt.keys().map(String::from).collect(),
                  }));
                }
                MonoType::Int | MonoType::Float | MonoType::Bool | MonoType::Arr(_) | MonoType::Bytes | MonoType::Duration | MonoType::Regexp | MonoType::String => {
                  list.push(Box::new(VarResult {
                    name: head.k,
                    package: package.clone(),
                  }));
                }
                _ => {}
            }

            walk(package, list, tail);
        }
    }
}

pub fn get_stdlib_functions() -> Vec<Box<dyn Completable>> {
    let env = imports().unwrap();
    let mut list = vec![];

    for (key, val) in env.values {
        println!("{} ->", key);

        walk(key, &mut list, val.expr);
    }

    list
}
