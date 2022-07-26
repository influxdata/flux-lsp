/// Flux transformations
///
/// The code here is not for visiting or analyzing flux, but for transforming AST
/// (and only AST).
use flux::ast;
use flux::ast::walk;

fn make_from_function(bucket: String, num: usize) -> ast::Statement {
    let from = ast::CallExpr {
        base: ast::BaseNode::default(),
        callee: ast::Expression::Identifier(ast::Identifier {
            base: ast::BaseNode::default(),
            name: "from".into(),
        }),
        arguments: vec![ast::Expression::Object(Box::new(
            ast::ObjectExpr {
                base: ast::BaseNode::default(),
                lbrace: vec![],
                with: None,
                properties: vec![flux::ast::Property {
                    base: ast::BaseNode::default(),
                    key: ast::PropertyKey::Identifier(
                        ast::Identifier {
                            base: ast::BaseNode::default(),
                            name: "bucket".into(),
                        },
                    ),
                    separator: vec![],
                    value: Some(ast::Expression::StringLit(
                        ast::StringLit {
                            base: ast::BaseNode::default(),
                            value: bucket.clone(),
                        },
                    )),
                    comma: vec![],
                }],
                rbrace: vec![],
            },
        ))],
        lparen: vec![],
        rparen: vec![],
    };

    let range = ast::Expression::PipeExpr(Box::new(
            ast::PipeExpr {
                argument: ast::Expression::Call(Box::new(from)),
                base: ast::BaseNode::default(),
                call: ast::CallExpr {
                    arguments: vec![ast::Expression::Object(
                        Box::new(ast::ObjectExpr {
                            base: ast::BaseNode::default(),
                            properties: vec![ast::Property {
                                base: ast::BaseNode::default(),
                                key: ast::PropertyKey::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "start".into(),
                                    },
                                ),
                                value: Some(
                                    ast::Expression::Member(Box::new(ast::MemberExpr {
                                        base: ast::BaseNode::default(),
                                        lbrack: vec![],
                                        rbrack: vec![],
                                        object: ast::Expression::Identifier(
                                            ast::Identifier {
                                                base: ast::BaseNode::default(),
                                                name: "v".into(),
                                            }
                                        ),
                                        property: ast::PropertyKey::Identifier(ast::Identifier {
                                            base: ast::BaseNode::default(),
                                            name: "timeRangeStart".into(),
                                        })
                                    }))
                                ),
                                comma: vec![],
                                separator: vec![],
                            },
                            ast::Property {
                                base: ast::BaseNode::default(),
                                key: ast::PropertyKey::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "stop".into(),
                                    },
                                ),
                                value: Some(
                                    ast::Expression::Member(Box::new(ast::MemberExpr {
                                        base: ast::BaseNode::default(),
                                        lbrack: vec![],
                                        rbrack: vec![],
                                        object: ast::Expression::Identifier(
                                            ast::Identifier {
                                                base: ast::BaseNode::default(),
                                                name: "v".into(),
                                            }
                                        ),
                                        property: ast::PropertyKey::Identifier(ast::Identifier {
                                            base: ast::BaseNode::default(),
                                            name: "timeRangeStop".into(),
                                        })
                                    }))
                                ),
                                comma: vec![],
                                separator: vec![],
                            }
                            ],
                            lbrace: vec![],
                            rbrace: vec![],
                            with: None,
                        }),
                    )],
                    base: ast::BaseNode::default(),
                    callee: ast::Expression::Identifier(
                        ast::Identifier {
                            base: ast::BaseNode::default(),
                            name: "range".into(),
                        },
                    ),
                    lparen: vec![],
                    rparen: vec![],
                },
            },
        ))
    ;

    let yield_expr = ast::ExprStmt {
        base: ast::BaseNode::default(),
        expression: ast::Expression::PipeExpr(Box::new(
            ast::PipeExpr {
                argument: range,
                base: ast::BaseNode::default(),
                call: ast::CallExpr {
                    arguments: vec![ast::Expression::Object(
                        Box::new(ast::ObjectExpr {
                            base: ast::BaseNode::default(),
                            properties: vec![ast::Property {
                                base: ast::BaseNode::default(),
                                key: ast::PropertyKey::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "name".into(),
                                    },
                                ),
                                value: Some(
                                    ast::Expression::StringLit(
                                        ast::StringLit {
                                            base:
                                                ast::BaseNode::default(
                                                ),
                                            value: format!(
                                                "{}-{}",
                                                bucket, num
                                            ),
                                        },
                                    ),
                                ),
                                comma: vec![],
                                separator: vec![],
                            }],
                            lbrace: vec![],
                            rbrace: vec![],
                            with: None,
                        }),
                    )],
                    base: ast::BaseNode::default(),
                    callee: ast::Expression::Identifier(
                        ast::Identifier {
                            base: ast::BaseNode::default(),
                            name: "yield".into(),
                        },
                    ),
                    lparen: vec![],
                    rparen: vec![],
                },
            },
        )),
    };

    ast::Statement::Expr(Box::new(yield_expr))
}

#[derive(Default)]
struct FromBucketVisitor {
    bucket: Option<String>,
}

impl<'a> walk::Visitor<'a> for FromBucketVisitor {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        match node {
            walk::Node::CallExpr(call) => {
                if let ast::Expression::Identifier(identifier) =
                    &call.callee
                {
                    if identifier.name == "from" {
                        call.arguments.iter().for_each(|argument| {
                            if let ast::Expression::Object(obj) = argument {
                                obj.properties.iter().for_each(|property| {
                                    if let ast::PropertyKey::Identifier(key) = &property.key {
                                        if key.name == "bucket" {
                                            if let Some(ast::Expression::StringLit(value)) = &property.value {
                                                self.bucket = Some(value.value.clone());
                                            }
                                        }
                                    }
                                })
                            }
                        });
                        false
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
            _ => true,
        }
    }
}

// check if the last expression is "yield"
fn has_yield(statement: ast::Expression) -> bool {
    match statement {
        ast::Expression::PipeExpr(pipe_expr) => {
            let call = pipe_expr.call.clone();
            if let ast::Expression::Identifier(identifier) =
                &call.callee
            {
                if identifier.name == "yield" {
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Find the correct `from` expression in a query ast
///
/// The logic follows this: we _only_ ever want to look at the last statement
/// in a query ast. Is it a `from` expression? Does it match the specified bucket?
/// If that expression is found, pull that expression out of the query and return
/// it. Otherwise, Otherwise, create a new `from() |> range() |> yield()` statement
/// and return it.
///
/// Why only the last one? Unless we're adding information about cursor position, we
/// have to make a choice on where the insertion needs to be. That choice is explicitly
/// "at the end of the file."
fn find_the_from(
    file: &mut ast::File,
    bucket: String,
) -> ast::Statement {
    // XXX: rockstar (13 Jun 2022) - This still has an issue where the last call in the
    // pipe is a yield. Appending to that statement will change the result.
    let last_statement = file.body.pop();
    match last_statement {
        Some(last_statement) => {
            if let ast::Statement::Expr(statement) =
                last_statement.clone()
            {
                let walker = walk::Node::ExprStmt(statement.as_ref());
                let mut visitor = FromBucketVisitor::default();

                ast::walk::walk(&mut visitor, walker);

                if let Some(name) = visitor.bucket {
                    if name == bucket {
                        return ast::Statement::Expr(statement);
                    }
                }

                file.body.push(last_statement);
                make_from_function(bucket, file.body.len())
            } else {
                file.body.push(last_statement);
                make_from_function(bucket, file.body.len())
            }
        }
        None => make_from_function(bucket, file.body.len()),
    }
}

/// Create a function used as then `fn` parameter of `filter`
///
/// This will return the ast equivalent of `(r) => r.{field} == "{value}"`.
fn make_flux_filter_function(
    field: String,
    value: String,
) -> ast::Expression {
    ast::Expression::Function(Box::new(ast::FunctionExpr {
        arrow: vec![],
        base: ast::BaseNode::default(),
        body: ast::FunctionBody::Expr(ast::Expression::Binary(
            Box::new(ast::BinaryExpr {
                base: ast::BaseNode::default(),
                left: ast::Expression::Member(Box::new(
                    ast::MemberExpr {
                        base: ast::BaseNode::default(),
                        lbrack: vec![],
                        rbrack: vec![],
                        object: ast::Expression::Identifier(
                            ast::Identifier {
                                base: ast::BaseNode::default(),
                                name: "r".into(),
                            },
                        ),
                        property: ast::PropertyKey::Identifier(
                            ast::Identifier {
                                base: ast::BaseNode::default(),
                                name: field,
                            },
                        ),
                    },
                )),
                right: ast::Expression::StringLit(ast::StringLit {
                    base: ast::BaseNode::default(),
                    value,
                }),
                operator: ast::Operator::EqualOperator,
            }),
        )),
        lparen: vec![],
        rparen: vec![],
        params: vec![ast::Property {
            base: ast::BaseNode::default(),
            key: ast::PropertyKey::Identifier(ast::Identifier {
                base: ast::BaseNode::default(),
                name: "r".into(),
            }),
            comma: vec![],
            separator: vec![],
            value: None,
        }],
    }))
}

pub(crate) fn inject_tag_filter(
    file: &ast::File,
    name: String,
    bucket: String,
) -> Result<ast::File, ()> {
    let mut ast = file.clone();

    let call: ast::Expression = if let ast::Statement::Expr(expr) =
        find_the_from(&mut ast, bucket)
    {
        expr.expression
    } else {
        return Err(());
    };

    ast.body.push(ast::Statement::Expr(
        Box::new(ast::ExprStmt {
            base: ast::BaseNode::default(),
            expression: ast::Expression::PipeExpr(Box::new(ast::PipeExpr {
                argument: call,
                base: ast::BaseNode::default(),
                call: ast::CallExpr {
                    arguments: vec![ast::Expression::Object(Box::new(ast::ObjectExpr {
                        base: ast::BaseNode::default(),
                        properties: vec![
                            ast::Property {
                                base: ast::BaseNode::default(),
                                key: ast::PropertyKey::Identifier(ast::Identifier {
                                    base: ast::BaseNode::default(),
                                    name: "fn".into(),
                                }),
                                value: Some(ast::Expression::Function(Box::new(ast::FunctionExpr{
                                    arrow: vec![],
                                    base: ast::BaseNode::default(),
                                    body: ast::FunctionBody::Expr(ast::Expression::Unary(Box::new(ast::UnaryExpr{
                                        base: ast::BaseNode::default(),
                                        argument: ast::Expression::Member(Box::new(ast::MemberExpr {
                                            base: ast::BaseNode::default(),
                                            lbrack: vec![],
                                            rbrack: vec![],
                                            object: ast::Expression::Identifier(ast::Identifier {
                                                base: ast::BaseNode::default(),
                                                name: "r".into(),
                                            }),
                                            property: ast::PropertyKey::Identifier(ast::Identifier {
                                                base: ast::BaseNode::default(),
                                                name,
                                            }),
                                        })),
                                        operator: ast::Operator::ExistsOperator,
                                    }))),
                                    lparen: vec![],
                                    rparen: vec![],
                                    params: vec![ast::Property {
                                        base: ast::BaseNode::default(),
                                        key: ast::PropertyKey::Identifier(ast::Identifier {
                                            base: ast::BaseNode::default(),
                                            name: "r".into(),
                                        }),
                                        comma: vec![],
                                        separator: vec![],
                                        value: None,
                                    }],
                                }))),
                                comma: vec![],
                                separator: vec![],
                            }
                        ],
                        lbrace: vec![],
                        rbrace: vec![],
                        with: None,
                    }))],
                    base: ast::BaseNode::default(),
                    callee: ast::Expression::Identifier(ast::Identifier {
                        base: ast::BaseNode::default(),
                        name: "filter".into(),
                    }),
                    lparen: vec![],
                    rparen: vec![],
                }
            }))
        })
    ));

    Ok(ast)
}

pub(crate) fn inject_field_filter(
    file: &ast::File,
    name: String,
    bucket: String,
) -> Result<ast::File, ()> {
    let mut ast = file.clone();

    let call: ast::Expression = if let ast::Statement::Expr(expr) =
        find_the_from(&mut ast, bucket)
    {
        expr.expression
    } else {
        return Err(());
    };

    ast.body.push(ast::Statement::Expr(Box::new(ast::ExprStmt {
        base: ast::BaseNode::default(),
        expression: ast::Expression::PipeExpr(Box::new(
            ast::PipeExpr {
                argument: call,
                base: ast::BaseNode::default(),
                call: ast::CallExpr {
                    arguments: vec![ast::Expression::Object(
                        Box::new(ast::ObjectExpr {
                            base: ast::BaseNode::default(),
                            properties: vec![ast::Property {
                                base: ast::BaseNode::default(),
                                key: ast::PropertyKey::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "fn".into(),
                                    },
                                ),
                                value: Some(
                                    make_flux_filter_function(
                                        "_field".into(),
                                        name,
                                    ),
                                ),
                                comma: vec![],
                                separator: vec![],
                            }],
                            lbrace: vec![],
                            rbrace: vec![],
                            with: None,
                        }),
                    )],
                    base: ast::BaseNode::default(),
                    callee: ast::Expression::Identifier(
                        ast::Identifier {
                            base: ast::BaseNode::default(),
                            name: "filter".into(),
                        },
                    ),
                    lparen: vec![],
                    rparen: vec![],
                },
            },
        )),
    })));
    Ok(ast)
}

pub(crate) fn inject_tag_value_filter(
    file: &ast::File,
    name: String,
    value: String,
    bucket: String,
) -> Result<ast::File, ()> {
    let mut ast = file.clone();

    let call: ast::Expression = if let ast::Statement::Expr(expr) =
        find_the_from(&mut ast, bucket)
    {
        expr.expression
    } else {
        return Err(());
    };

    ast.body.push(ast::Statement::Expr(Box::new(ast::ExprStmt {
        base: ast::BaseNode::default(),
        expression: ast::Expression::PipeExpr(Box::new(
            ast::PipeExpr {
                argument: call,
                base: ast::BaseNode::default(),
                call: ast::CallExpr {
                    arguments: vec![ast::Expression::Object(
                        Box::new(ast::ObjectExpr {
                            base: ast::BaseNode::default(),
                            properties: vec![ast::Property {
                                base: ast::BaseNode::default(),
                                key: ast::PropertyKey::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "fn".into(),
                                    },
                                ),
                                value: Some(
                                    make_flux_filter_function(
                                        name, value,
                                    ),
                                ),
                                comma: vec![],
                                separator: vec![],
                            }],
                            lbrace: vec![],
                            rbrace: vec![],
                            with: None,
                        }),
                    )],
                    base: ast::BaseNode::default(),
                    callee: ast::Expression::Identifier(
                        ast::Identifier {
                            base: ast::BaseNode::default(),
                            name: "filter".into(),
                        },
                    ),
                    lparen: vec![],
                    rparen: vec![],
                },
            },
        )),
    })));
    Ok(ast)
}

pub(crate) fn inject_measurement_filter(
    file: &ast::File,
    name: String,
    bucket: String,
) -> Result<ast::File, ()> {
    let mut ast = file.clone();

    let call: ast::Expression = if let ast::Statement::Expr(expr) =
        find_the_from(&mut ast, bucket.clone())
    {
        expr.expression
    } else {
        return Err(());
    };

    let last_statement: ast::Expression = call.clone();
    if has_yield(last_statement) {
        // TODO (chunchun): remove the yield
    }

    let filter_expr =
        ast::Expression::PipeExpr(Box::new(ast::PipeExpr {
            argument: call,
            base: ast::BaseNode::default(),
            call: ast::CallExpr {
                arguments: vec![ast::Expression::Object(Box::new(
                    ast::ObjectExpr {
                        base: ast::BaseNode::default(),
                        properties: vec![ast::Property {
                            base: ast::BaseNode::default(),
                            key: ast::PropertyKey::Identifier(
                                ast::Identifier {
                                    base: ast::BaseNode::default(),
                                    name: "fn".into(),
                                },
                            ),
                            value: Some(make_flux_filter_function(
                                "_measurement".into(),
                                name,
                            )),
                            comma: vec![],
                            separator: vec![],
                        }],
                        lbrace: vec![],
                        rbrace: vec![],
                        with: None,
                    },
                ))],
                base: ast::BaseNode::default(),
                callee: ast::Expression::Identifier(
                    ast::Identifier {
                        base: ast::BaseNode::default(),
                        name: "filter".into(),
                    },
                ),
                lparen: vec![],
                rparen: vec![],
            },
        }));

    let yield_expr = ast::ExprStmt {
        base: ast::BaseNode::default(),
        expression: ast::Expression::PipeExpr(Box::new(
            ast::PipeExpr {
                argument: filter_expr,
                base: ast::BaseNode::default(),
                call: ast::CallExpr {
                    arguments: vec![ast::Expression::Object(
                        Box::new(ast::ObjectExpr {
                            base: ast::BaseNode::default(),
                            properties: vec![ast::Property {
                                base: ast::BaseNode::default(),
                                key: ast::PropertyKey::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "name".into(),
                                    },
                                ),
                                value: Some(
                                    ast::Expression::StringLit(
                                        ast::StringLit {
                                            base:
                                                ast::BaseNode::default(
                                                ),
                                            value: format!(
                                                "{}-{}",
                                                bucket,
                                                ast.body.len(),
                                            ),
                                        },
                                    ),
                                ),
                                comma: vec![],
                                separator: vec![],
                            }],
                            lbrace: vec![],
                            rbrace: vec![],
                            with: None,
                        }),
                    )],
                    base: ast::BaseNode::default(),
                    callee: ast::Expression::Identifier(
                        ast::Identifier {
                            base: ast::BaseNode::default(),
                            name: "yield".into(),
                        },
                    ),
                    lparen: vec![],
                    rparen: vec![],
                },
            },
        )),
    };

    ast.body.push(ast::Statement::Expr(Box::new(yield_expr)));

    Ok(ast)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // When the last statement isn't a `from` call, return a new `from` call.
    #[test]
    fn test_find_the_from_last_statement_not_from() {
        let fluxscript = r#"from(bucket: "my-bucket")
        
a = 0"#;
        let mut ast =
            flux::parser::parse_string("".into(), &fluxscript);

        let from = find_the_from(&mut ast, "my-bucket".into());

        assert_eq!(
            from.base().location,
            ast::SourceLocation {
                start: ast::Position { line: 0, column: 0 },
                end: ast::Position { line: 0, column: 0 },
                file: None,
                source: None
            }
        );
        assert_eq!(2, ast.body.len());
    }

    // When the query is empty, return a new `from` call.
    #[test]
    fn test_find_the_from_empty_query() {
        let fluxscript = r#""#;
        let mut ast =
            flux::parser::parse_string("".into(), &fluxscript);

        let from = find_the_from(&mut ast, "my-bucket".into());

        assert_eq!(
            from.base().location,
            ast::SourceLocation {
                start: ast::Position { line: 0, column: 0 },
                end: ast::Position { line: 0, column: 0 },
                file: None,
                source: None
            }
        );
        assert_eq!(0, ast.body.len());

        ast.body.push(from);
        let expected = r#"from(bucket: "my-bucket") |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&ast).unwrap()
        );
    }

    // When the last `from` call isn't the right bucket, return a new `from` call.
    #[test]
    fn test_find_the_from_not_existing_from() {
        let fluxscript = r#"from(bucket: "my-bucket")"#;
        let mut ast =
            flux::parser::parse_string("".into(), &fluxscript);

        let from = find_the_from(&mut ast, "my-new-bucket".into());

        assert_eq!(
            from.base().location,
            ast::SourceLocation {
                start: ast::Position { line: 0, column: 0 },
                end: ast::Position { line: 0, column: 0 },
                file: None,
                source: None
            }
        );
        assert_eq!(1, ast.body.len());
    }

    // When the last `from` call is the correct bucket, return that call.
    #[test]
    fn test_find_the_from_from_found() {
        let fluxscript = r#"from(bucket: "my-bucket")"#;
        let mut ast =
            flux::parser::parse_string("".into(), &fluxscript);

        let from = find_the_from(&mut ast, "my-bucket".into());

        assert_eq!(
            from.base().location,
            ast::SourceLocation {
                start: ast::Position { line: 1, column: 1 },
                end: ast::Position {
                    line: 1,
                    column: 26
                },
                file: None,
                source: Some(r#"from(bucket: "my-bucket")"#.into())
            }
        );
        assert_eq!(0, ast.body.len());
    }

    // When the expected `from` is nested in a pipe expr, walk down
    // into it to find it.
    #[test]
    fn test_find_the_from_from_with_pipe_expr() {
        let fluxscript = r#"from(bucket: "my-bucket")
  |> range(start: v.timeRangeStart, stop: v.timeRangeStop)"#;
        let mut ast =
            flux::parser::parse_string("".into(), &fluxscript);

        let from = find_the_from(&mut ast, "my-bucket".into());

        assert_eq!(
            from.base().location,
            ast::SourceLocation {
                start: ast::Position { line: 1, column: 1 },
                end: ast::Position {
                    line: 2,
                    column: 59
                },
                file: None,
                source: Some(
                    r#"from(bucket: "my-bucket")
  |> range(start: v.timeRangeStart, stop: v.timeRangeStop)"#
                        .into()
                )
            }
        );
        assert_eq!(0, ast.body.len());
    }

    #[test]
    fn test_inject_tag_key() {
        let fluxscript = r#"from(bucket: "my-bucket")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed =
            inject_tag_filter(&ast, "cpu".into(), "my-bucket".into())
                .unwrap();

        let expected = r#"from(bucket: "my-bucket") |> filter(fn: (r) => exists r.cpu)
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }

    #[test]
    fn test_inject_tag_key_no_from() {
        let fluxscript = r#""#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed =
            inject_tag_filter(&ast, "cpu".into(), "my-bucket".into())
                .unwrap();

        let expected = r#"from(bucket: "my-bucket") |> range(start: v.timeRangeStart, stop: v.timeRangeStop) |> filter(fn: (r) => exists r.cpu)
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }

    #[test]
    fn test_inject_tag_value_filter() {
        let fluxscript = r#"from(bucket: "my-bucket")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed = inject_tag_value_filter(
            &ast,
            "myTag".into(),
            "myTagValue".into(),
            "my-bucket".into(),
        )
        .unwrap();

        let expected = r#"from(bucket: "my-bucket") |> filter(fn: (r) => r.myTag == "myTagValue")
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }

    #[test]
    fn test_inject_tag_value_filter_no_from() {
        let fluxscript = r#""#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed = inject_tag_value_filter(
            &ast,
            "myTag".into(),
            "myTagValue".into(),
            "my-bucket".into(),
        )
        .unwrap();

        let expected = r#"from(bucket: "my-bucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.myTag == "myTagValue")
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }

    #[test]
    fn test_inject_field_filter() {
        let fluxscript = r#"from(bucket: "my-bucket")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed = inject_field_filter(
            &ast,
            "myField".into(),
            "my-bucket".into(),
        )
        .unwrap();

        let expected = r#"from(bucket: "my-bucket") |> filter(fn: (r) => r._field == "myField")
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }

    #[test]
    fn test_inject_measurement_filter() {
        let fluxscript = r#"from(bucket: "my-bucket")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed = inject_measurement_filter(
            &ast,
            "myMeasurement".into(),
            "my-bucket".into(),
        )
        .unwrap();

        let expected = r#"from(bucket: "my-bucket") |> filter(fn: (r) => r._measurement == "myMeasurement")
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }

    #[test]
    fn test_inject_measurement_append_yield() {
        let fluxscript = r#"from(bucket: "my-bucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "test")
"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed = inject_measurement_filter(
            &ast,
            "myMeasurement".into(),
            "my-new-bucket".into(),
        )
        .unwrap();

        let expected = r#"from(bucket: "my-bucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "test")
from(bucket: "my-new-bucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> yield(name: "my-new-bucket-1")
"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }
}
