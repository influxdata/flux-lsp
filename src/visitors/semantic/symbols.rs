#![allow(deprecated)]

use flux::semantic::nodes::{self, Expression};
use flux::semantic::walk::{Node, Visitor};
use lspower::lsp;

fn parse_variable_assignment(
    uri: lsp::Url,
    node: Node,
    va: &nodes::VariableAssgn,
) -> Vec<lsp::SymbolInformation> {
    if let Expression::Function(f) = va.init.clone() {
        vec![lsp::SymbolInformation {
            kind: lsp::SymbolKind::FUNCTION,
            name: va.id.name.to_string(),
            location: lsp::Location {
                uri: uri.clone(),
                range: node.loc().clone().into(),
            },
            tags: None,
            deprecated: None,
            container_name: None,
        }]
        .into_iter()
        .chain(f.params.into_iter().map(|param| {
            lsp::SymbolInformation {
                kind: lsp::SymbolKind::VARIABLE,
                name: param.key.name.to_string(),
                location: lsp::Location {
                    uri: uri.clone(),
                    range: param.loc.into(),
                },
                tags: None,
                deprecated: None,
                container_name: None,
            }
        }))
        .collect()
    } else {
        vec![lsp::SymbolInformation {
            kind: lsp::SymbolKind::VARIABLE,
            name: va.id.name.to_string(),
            location: lsp::Location {
                uri,
                range: node.loc().clone().into(),
            },
            tags: None,
            deprecated: None,
            container_name: None,
        }]
    }
}

fn parse_call_expression(
    uri: lsp::Url,
    c: &nodes::CallExpr,
) -> Vec<lsp::SymbolInformation> {
    let initial_symbols =
        if let Expression::Identifier(ident) = c.callee.clone() {
            vec![lsp::SymbolInformation {
                kind: lsp::SymbolKind::FUNCTION,
                name: ident.name.to_string(),
                location: lsp::Location {
                    uri: uri.clone(),
                    range: c.loc.clone().into(),
                },
                tags: None,
                deprecated: None,
                container_name: None,
            }]
        } else {
            vec![]
        };

    initial_symbols
        .into_iter()
        .chain(c.arguments.clone().into_iter().map(|arg| {
            if let Expression::Function(_) = arg.value {
                lsp::SymbolInformation {
                    kind: lsp::SymbolKind::FUNCTION,
                    name: arg.key.name.to_string(),
                    location: lsp::Location {
                        uri: uri.clone(),
                        range: arg.loc.into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                }
            } else {
                lsp::SymbolInformation {
                    kind: lsp::SymbolKind::VARIABLE,
                    name: arg.key.name.to_string(),
                    location: lsp::Location {
                        uri: uri.clone(),
                        range: arg.loc.into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                }
            }
        }))
        .collect()
}

fn parse_binary_expression(
    uri: lsp::Url,
    be: &nodes::BinaryExpr,
) -> Vec<lsp::SymbolInformation> {
    #[allow(clippy::expect_used)]
    vec![
        if let Expression::Identifier(ident) = be.left.clone() {
            Some(lsp::SymbolInformation {
                kind: lsp::SymbolKind::VARIABLE,
                name: ident.name.to_string(),
                location: lsp::Location {
                    uri: uri.clone(),
                    range: ident.loc.into(),
                },
                tags: None,
                deprecated: None,
                container_name: None,
            })
        } else {
            None
        },
        if let Expression::Identifier(ident) = be.right.clone() {
            Some(lsp::SymbolInformation {
                kind: lsp::SymbolKind::VARIABLE,
                name: ident.name.to_string(),
                location: lsp::Location {
                    uri,
                    range: ident.loc.into(),
                },
                tags: None,
                deprecated: None,
                container_name: None,
            })
        } else {
            None
        },
    ]
    .into_iter()
    .filter(|item| !item.is_none())
    .map(|item| item.expect("Previous filter call failed"))
    .collect()
}

pub struct SymbolsVisitor<'a> {
    pub symbols: Vec<lsp::SymbolInformation>,
    pub uri: lsp::Url,
    pub path: Vec<Node<'a>>,
}

impl<'a> SymbolsVisitor<'a> {
    pub fn new(uri: lsp::Url) -> SymbolsVisitor<'a> {
        SymbolsVisitor {
            path: vec![],
            symbols: vec![],
            uri,
        }
    }
}

impl<'a> SymbolsVisitor<'a> {}

impl<'a> Visitor<'a> for SymbolsVisitor<'a> {
    fn done(&mut self, _: Node<'a>) {
        self.path.pop();
    }

    fn visit(&mut self, node: Node<'a>) -> bool {
        let uri = self.uri.clone();

        self.path.push(node);

        match node {
            Node::VariableAssgn(va) => {
                let list = parse_variable_assignment(uri, node, va);

                for si in list {
                    self.symbols.push(si);
                }
            }
            Node::CallExpr(c) => {
                let list = parse_call_expression(uri, c);

                for si in list {
                    self.symbols.push(si);
                }
            }
            Node::BinaryExpr(be) => {
                let list = parse_binary_expression(uri, be);

                for si in list {
                    self.symbols.push(si);
                }
            }
            Node::MemberExpr(me) => {
                if let Some(source) = me.loc.source.clone() {
                    self.symbols.push(lsp::SymbolInformation {
                        kind: lsp::SymbolKind::OBJECT,
                        name: source,
                        location: lsp::Location {
                            uri,
                            range: me.loc.clone().into(),
                        },
                        tags: None,
                        deprecated: None,
                        container_name: None,
                    });
                }
            }
            Node::FloatLit(num) => {
                self.symbols.push(lsp::SymbolInformation {
                    kind: lsp::SymbolKind::NUMBER,
                    name: num.value.to_string(),
                    location: lsp::Location {
                        uri,
                        range: num.loc.clone().into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                });
                return false;
            }
            Node::IntegerLit(num) => {
                self.symbols.push(lsp::SymbolInformation {
                    kind: lsp::SymbolKind::NUMBER,
                    name: num.value.to_string(),
                    location: lsp::Location {
                        uri,
                        range: num.loc.clone().into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                });
                return false;
            }
            Node::DateTimeLit(d) => {
                self.symbols.push(lsp::SymbolInformation {
                    kind: lsp::SymbolKind::CONSTANT,
                    name: d.value.to_string(),
                    location: lsp::Location {
                        uri,
                        range: d.loc.clone().into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                });
                return false;
            }
            Node::BooleanLit(b) => {
                self.symbols.push(lsp::SymbolInformation {
                    kind: lsp::SymbolKind::BOOLEAN,
                    name: b.value.to_string(),
                    location: lsp::Location {
                        uri,
                        range: b.loc.clone().into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                });
                return false;
            }
            Node::StringLit(s) => {
                self.symbols.push(lsp::SymbolInformation {
                    kind: lsp::SymbolKind::STRING,
                    name: s.value.clone(),
                    location: lsp::Location {
                        uri,
                        range: s.loc.clone().into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                });
                return false;
            }
            Node::ArrayExpr(a) => {
                self.symbols.push(lsp::SymbolInformation {
                    kind: lsp::SymbolKind::ARRAY,
                    name: String::from("[]"),
                    location: lsp::Location {
                        uri,
                        range: a.loc.clone().into(),
                    },
                    tags: None,
                    deprecated: None,
                    container_name: None,
                })
            }
            _ => (),
        }
        true
    }
}
