/// Flux transformations
///
/// The code here is not for visiting or analyzing flux, but for transforming AST
/// (and only AST).
use flux::ast;

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
) -> Result<ast::File, ()> {
    if let Some(statement) = file
        .body
        .iter()
        .filter(|node| {
            if let ast::Statement::Expr(_stmt) = node {
                return true;
            }
            false
        })
        .last()
    {
        let mut new_ast = file.clone();
        new_ast.body.retain(|x| x != statement);

        let call: &ast::Expression =
            if let ast::Statement::Expr(expr) = statement {
                &expr.expression
            } else {
                return Err(());
            };

        new_ast.body.push(ast::Statement::Expr(
        Box::new(ast::ExprStmt {
            base: ast::BaseNode::default(),
            expression: ast::Expression::PipeExpr(Box::new(ast::PipeExpr {
                argument: call.clone(),
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
        return Ok(new_ast);
    }
    Err(())
}

pub(crate) fn inject_tag_value_filter(
    file: &ast::File,
    name: String,
    value: String,
) -> Result<ast::File, ()> {
    if let Some(statement) = file
        .body
        .iter()
        .filter(|node| {
            if let ast::Statement::Expr(_stmt) = node {
                return true;
            }
            false
        })
        .last()
    {
        let mut new_ast = file.clone();
        new_ast.body.retain(|x| x != statement);

        let call: &ast::Expression =
            if let ast::Statement::Expr(expr) = statement {
                &expr.expression
            } else {
                return Err(());
            };

        new_ast.body.push(ast::Statement::Expr(
            Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(ast::PipeExpr {
                    argument: call.clone(),
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
                                    value: Some(make_flux_filter_function(name, value)),
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
        return Ok(new_ast);
    }
    Err(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_tag_key() {
        let fluxscript = r#"from(bucket: "my-bucket")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let transformed =
            inject_tag_filter(&ast, "cpu".into()).unwrap();

        let expected = r#"from(bucket: "my-bucket") |> filter(fn: (r) => exists r.cpu)"#;
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
        )
        .unwrap();

        let expected = r#"from(bucket: "my-bucket") |> filter(fn: (r) => r.myTag == "myTagValue")"#;
        assert_eq!(
            expected,
            flux::formatter::convert_to_string(&transformed).unwrap()
        );
    }
}
