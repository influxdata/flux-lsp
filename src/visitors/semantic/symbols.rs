#![allow(deprecated)]

use flux::semantic::nodes::{self, Expression};
use flux::semantic::walk::{Node, Visitor};
use tower_lsp::lsp_types as lsp;

fn parse_variable_assignment(
    uri: lsp::Url,
    node: Node,
    va: &nodes::VariableAssgn,
) -> Vec<lsp::SymbolInformation> {
    let mut result = vec![];

    if let Expression::Function(f) = va.init.clone() {
        result.push(lsp::SymbolInformation {
            kind: lsp::SymbolKind::FUNCTION,
            name: va.id.name.to_string(),
            location: lsp::Location {
                uri: uri.clone(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: node.loc().start.line - 1,
                        character: node.loc().start.column - 1,
                    },
                    end: lsp::Position {
                        line: node.loc().end.line - 1,
                        character: node.loc().end.column - 1,
                    },
                },
            },
            tags: None,
            deprecated: None,
            container_name: None,
        });

        for param in f.params {
            result.push(lsp::SymbolInformation {
                kind: lsp::SymbolKind::VARIABLE,
                name: param.key.name.to_string(),
                location: lsp::Location {
                    uri: uri.clone(),
                    range: lsp::Range {
                        start: lsp::Position {
                            line: param.loc.start.line - 1,
                            character: param.loc.start.column - 1,
                        },
                        end: lsp::Position {
                            line: param.loc.end.line - 1,
                            character: param.loc.end.column - 1,
                        },
                    },
                },
                tags: None,
                deprecated: None,
                container_name: None,
            });
        }
    } else {
        result.push(lsp::SymbolInformation {
            kind: lsp::SymbolKind::VARIABLE,
            name: va.id.name.to_string(),
            location: lsp::Location {
                uri,
                range: lsp::Range {
                    start: lsp::Position {
                        line: node.loc().start.line - 1,
                        character: node.loc().start.column - 1,
                    },
                    end: lsp::Position {
                        line: node.loc().end.line - 1,
                        character: node.loc().end.column - 1,
                    },
                },
            },
            tags: None,
            deprecated: None,
            container_name: None,
        })
    }

    result
}

fn parse_call_expression(
    uri: lsp::Url,
    c: &nodes::CallExpr,
) -> Vec<lsp::SymbolInformation> {
    let mut result = vec![];

    if let Expression::Identifier(ident) = c.callee.clone() {
        result.push(lsp::SymbolInformation {
            kind: lsp::SymbolKind::FUNCTION,
            name: ident.name.to_string(),
            location: lsp::Location {
                uri: uri.clone(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: c.loc.start.line - 1,
                        character: c.loc.start.column - 1,
                    },
                    end: lsp::Position {
                        line: c.loc.end.line - 1,
                        character: c.loc.end.column - 1,
                    },
                },
            },
            tags: None,
            deprecated: None,
            container_name: None,
        })
    }

    for arg in c.arguments.clone() {
        if let Expression::Function(_) = arg.value {
            result.push(lsp::SymbolInformation {
                kind: lsp::SymbolKind::FUNCTION,
                name: arg.key.name.to_string(),
                location: lsp::Location {
                    uri: uri.clone(),
                    range: lsp::Range {
                        start: lsp::Position {
                            line: arg.loc.start.line - 1,
                            character: arg.loc.start.column - 1,
                        },
                        end: lsp::Position {
                            line: arg.loc.end.line - 1,
                            character: arg.loc.end.column - 1,
                        },
                    },
                },
                tags: None,
                deprecated: None,
                container_name: None,
            });
        } else {
            result.push(lsp::SymbolInformation {
                kind: lsp::SymbolKind::VARIABLE,
                name: arg.key.name.to_string(),
                location: lsp::Location {
                    uri: uri.clone(),
                    range: lsp::Range {
                        start: lsp::Position {
                            line: arg.loc.start.line - 1,
                            character: arg.loc.start.column - 1,
                        },
                        end: lsp::Position {
                            line: arg.loc.end.line - 1,
                            character: arg.loc.end.column - 1,
                        },
                    },
                },
                tags: None,
                deprecated: None,
                container_name: None,
            });
        }
    }

    result
}

fn parse_binary_expression(
    uri: lsp::Url,
    be: &nodes::BinaryExpr,
) -> Vec<lsp::SymbolInformation> {
    let mut result = vec![];

    if let Expression::Identifier(ident) = be.left.clone() {
        result.push(lsp::SymbolInformation {
            kind: lsp::SymbolKind::VARIABLE,
            name: ident.name.to_string(),
            location: lsp::Location {
                uri: uri.clone(),
                range: lsp::Range {
                    start: lsp::Position {
                        line: ident.loc.start.line - 1,
                        character: ident.loc.start.column - 1,
                    },
                    end: lsp::Position {
                        line: ident.loc.end.line - 1,
                        character: ident.loc.end.column - 1,
                    },
                },
            },
            tags: None,
            deprecated: None,
            container_name: None,
        })
    }

    if let Expression::Identifier(ident) = be.right.clone() {
        result.push(lsp::SymbolInformation {
            kind: lsp::SymbolKind::VARIABLE,
            name: ident.name.to_string(),
            location: lsp::Location {
                uri,
                range: lsp::Range {
                    start: lsp::Position {
                        line: ident.loc.start.line - 1,
                        character: ident.loc.start.column - 1,
                    },
                    end: lsp::Position {
                        line: ident.loc.end.line - 1,
                        character: ident.loc.end.column - 1,
                    },
                },
            },
            tags: None,
            deprecated: None,
            container_name: None,
        })
    }

    result
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
                            range: lsp::Range {
                                start: lsp::Position {
                                    line: me.loc.start.line - 1,
                                    character: me.loc.start.column
                                        - 1,
                                },
                                end: lsp::Position {
                                    line: me.loc.end.line - 1,
                                    character: me.loc.end.column - 1,
                                },
                            },
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
                        range: lsp::Range {
                            start: lsp::Position {
                                line: num.loc.start.line - 1,
                                character: num.loc.start.column - 1,
                            },
                            end: lsp::Position {
                                line: num.loc.end.line - 1,
                                character: num.loc.end.column - 1,
                            },
                        },
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
                        range: lsp::Range {
                            start: lsp::Position {
                                line: num.loc.start.line - 1,
                                character: num.loc.start.column - 1,
                            },
                            end: lsp::Position {
                                line: num.loc.end.line - 1,
                                character: num.loc.end.column - 1,
                            },
                        },
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
                        range: lsp::Range {
                            start: lsp::Position {
                                line: d.loc.start.line - 1,
                                character: d.loc.start.column - 1,
                            },
                            end: lsp::Position {
                                line: d.loc.end.line - 1,
                                character: d.loc.end.column - 1,
                            },
                        },
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
                        range: lsp::Range {
                            start: lsp::Position {
                                line: b.loc.start.line - 1,
                                character: b.loc.start.column - 1,
                            },
                            end: lsp::Position {
                                line: b.loc.end.line - 1,
                                character: b.loc.end.column - 1,
                            },
                        },
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
                        range: lsp::Range {
                            start: lsp::Position {
                                line: s.loc.start.line - 1,
                                character: s.loc.start.column - 1,
                            },
                            end: lsp::Position {
                                line: s.loc.end.line - 1,
                                character: s.loc.end.column - 1,
                            },
                        },
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
                        range: lsp::Range {
                            start: lsp::Position {
                                line: a.loc.start.line - 1,
                                character: a.loc.start.column - 1,
                            },
                            end: lsp::Position {
                                line: a.loc.end.line - 1,
                                character: a.loc.end.column - 1,
                            },
                        },
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
