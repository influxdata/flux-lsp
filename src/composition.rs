/// Composition functionality
///
/// This module covers all the functionality that comes from the Composition feature of the
/// LSP server. It's spec can be found in the docs/ folder of source control.
///
/// This module _only_ operates on an AST. It will never operate on semantic graph.
use flux::ast;

static YIELD_IDENTIFIER: &str = "_editor_composition";

macro_rules! from {
    ($bucket_name:expr) => {
        ast::CallExpr {
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
                                value: $bucket_name,  // BUCKET GOES HERE
                            },
                        )),
                        comma: vec![],
                    }],
                    rbrace: vec![],
                },
            ))],
            lparen: vec![],
            rparen: vec![],
        }
    };
}

macro_rules! range {
    () => {
        ast::CallExpr {
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
        }
    }
}

macro_rules! exists {
    ($key:expr) => {
        ast::Expression::Unary(Box::new(ast::UnaryExpr {
            base: ast::BaseNode::default(),
            operator: ast::Operator::ExistsOperator,
            argument: ast::Expression::Member(Box::new(
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
                            name: $key,
                        },
                    ),
                },
            )),
        }))
    };
}

macro_rules! binary_eq_expr {
    ($key:expr, $value:expr) => {
        ast::Expression::Binary(Box::new(ast::BinaryExpr {
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
                            name: $key,
                        },
                    ),
                },
            )),
            right: ast::Expression::StringLit(ast::StringLit {
                base: ast::BaseNode::default(),
                value: $value,
            }),
            operator: ast::Operator::EqualOperator,
        }))
    };
}

fn chained_exists_expr(
    operator: ast::LogicalOperator,
    values: &[String],
) -> Result<ast::Expression, ()> {
    match values {
        [] => Err(()),
        [head] => Ok(exists!(head.to_string())),
        [head, ..] => {
            if let Ok(right) = chained_exists_expr(
                operator.clone(),
                &values[1..].to_vec(),
            ) {
                Ok(ast::Expression::Logical(Box::new(
                    ast::LogicalExpr {
                        base: ast::BaseNode::default(),
                        left: exists!(head.to_string()),
                        right,
                        operator,
                    },
                )))
            } else {
                Err(())
            }
        }
    }
}

/// Returns the logical expr, which are predicates joined by the operator.
///
/// # Arguments
/// * `operator` - ast::LogicalOperator used to join predicates.
/// * `key` - the string used in all of the predicates. key = value
/// * `values` - pointer to vector slice of strings, to be used as each value in the predicates.
///
/// logical_expr() is a recursive function, with an unknown length (at compile time) of the values vector.
/// As such, the filter! macro can be compile time (since it only pass the pointer to values).
/// Then this logical_expr() cannot be a macro, because has an unknown runtime recursive depth.
/// Yet the lower binary_eq_expr! can still be a macro.
fn chained_binary_eq_expr(
    operator: ast::LogicalOperator,
    keys: &[String],
    values: &[String],
) -> Result<ast::Expression, ()> {
    match (keys, values) {
        ([key], [value]) => {
            Ok(binary_eq_expr!(key.to_owned(), value.to_owned()))
        }
        ([key, ..], [value, ..]) => {
            if let Ok(right) = chained_binary_eq_expr(
                operator.clone(),
                &keys[1..],
                &values[1..],
            ) {
                Ok(ast::Expression::Logical(Box::new(
                    ast::LogicalExpr {
                        base: ast::BaseNode::default(),
                        left: binary_eq_expr!(
                            key.to_owned(),
                            value.to_owned()
                        ),
                        right,
                        operator,
                    },
                )))
            } else {
                Err(())
            }
        }
        _ => Err(()),
    }
}

macro_rules! filter {
    ($values:expr, $operator:expr) => {
        filter!(None, $values, $operator, chained_exists_expr($operator, $values).unwrap())
    };
    ($key:expr, $values:expr, $operator:expr) => {
        filter!($key, $values, $operator, chained_binary_eq_expr($operator, $key, $values).unwrap())
    };
    ($key:expr, $values:expr, $operator:expr, $funBody:expr) => {
        ast::CallExpr {
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
                            ast::Expression::Function(Box::new(ast::FunctionExpr {
                                arrow: vec![],
                                base: ast::BaseNode::default(),
                                body: ast::FunctionBody::Expr($funBody),
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
        }
    };
}

macro_rules! yield_ {
    () => {
        ast::CallExpr {
            arguments: vec![ast::Expression::Object(Box::new(
                ast::ObjectExpr {
                    base: ast::BaseNode::default(),
                    properties: vec![ast::Property {
                        base: ast::BaseNode::default(),
                        key: ast::PropertyKey::Identifier(
                            ast::Identifier {
                                base: ast::BaseNode::default(),
                                name: "name".into(),
                            },
                        ),
                        value: Some(ast::Expression::StringLit(
                            ast::StringLit {
                                base: ast::BaseNode::default(),
                                value: YIELD_IDENTIFIER.into(),
                            },
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
            callee: ast::Expression::Identifier(ast::Identifier {
                base: ast::BaseNode::default(),
                name: "yield".into(),
            }),
            lparen: vec![],
            rparen: vec![],
        }
    };
}

macro_rules! pipe {
    ($a:expr, $b:expr) => {
        ast::PipeExpr {
            argument: $a,
            base: ast::BaseNode::default(),
            call: $b,
        }
    };
}

enum SchemaItem {
    Measurement(String),
    Field(String),
    Tag(String),
    TagValue(String, String),
}

#[derive(Default)]
struct FilterDecomposed {
    items: Vec<SchemaItem>,
}

/// Analyze a query, understanding the various filters applied.
///
/// This struct is essentially a visitor, so it only provides a view into an existing
/// Composition statement, it does not make changes.
#[derive(Default)]
struct CompositionQueryAnalyzer {
    bucket: String,
    filters: Vec<FilterDecomposed>,
}

impl CompositionQueryAnalyzer {
    fn analyze(&mut self, statement: ast::ExprStmt) {
        ast::walk::walk(
            self,
            flux::ast::walk::Node::from_stmt(&ast::Statement::Expr(
                Box::new(statement),
            )),
        );
    }

    fn build(&mut self) -> ast::PipeExpr {
        fn to_strings(items: &[SchemaItem]) -> Vec<String> {
            items
                .iter()
                .filter_map(|item| match &item {
                    SchemaItem::Measurement(m) => Some(m.to_owned()),
                    SchemaItem::Field(f) => Some(f.to_owned()),
                    SchemaItem::Tag(t) => Some(t.to_owned()),
                    SchemaItem::TagValue(_, _) => None,
                })
                .collect()
        }

        let init = ast::Expression::PipeExpr(Box::new(pipe!(
            ast::Expression::Call(Box::new(from!(self
                .bucket
                .to_owned()))),
            range!()
        )));

        let done = self.filters.iter().fold(
            init,
            |inner, filter_decomposed| match &filter_decomposed.items
                [0]
            {
                SchemaItem::Measurement(measurement) => {
                    ast::Expression::PipeExpr(Box::new(pipe!(
                        inner,
                        filter!(
                            &["_measurement".to_string()],
                            &[measurement.to_owned()],
                            ast::LogicalOperator::OrOperator
                        )
                    )))
                }
                SchemaItem::Field(_) => {
                    ast::Expression::PipeExpr(Box::new(pipe!(
                        inner,
                        filter!(
                            &vec![
                                "_field".to_string();
                                filter_decomposed.items.len()
                            ],
                            &to_strings(&filter_decomposed.items),
                            ast::LogicalOperator::OrOperator
                        )
                    )))
                }
                SchemaItem::Tag(_) => {
                    ast::Expression::PipeExpr(Box::new(pipe!(
                        inner,
                        filter!(
                            &to_strings(&filter_decomposed.items),
                            ast::LogicalOperator::AndOperator
                        )
                    )))
                }
                SchemaItem::TagValue(_, _) => {
                    let (filter_keys, filter_values) =
                        filter_decomposed.items.iter().fold(
                            (vec![], vec![]),
                            |(keys, values), item| match &item {
                                SchemaItem::TagValue(key, value) => (
                                    [keys, vec![key.to_owned()]]
                                        .concat(),
                                    [values, vec![value.to_owned()]]
                                        .concat(),
                                ),
                                _ => (keys, values),
                            },
                        );
                    ast::Expression::PipeExpr(Box::new(pipe!(
                        inner,
                        filter!(
                            &filter_keys,
                            &filter_values,
                            ast::LogicalOperator::AndOperator
                        )
                    )))
                }
            },
        );

        pipe!(done, yield_!())
    }
}

impl<'a> ast::walk::Visitor<'a> for CompositionQueryAnalyzer {
    fn visit(&mut self, node: ast::walk::Node<'a>) -> bool {
        // Because we own the entire implementation of the Composition query statement, we can be super naive
        // about what the shape of these functions looks like. If the implementation ever gets more complex, than
        // we can short circuit execution in the matcher to prevent recursing into obvious dead-ends.
        match node {
            ast::walk::Node::CallExpr(call_expr) => {
                if let ast::Expression::Identifier(identifier) =
                    &call_expr.callee
                {
                    match identifier.name.as_str() {
                        "from" => {
                            if let ast::Expression::Object(
                                object_expr,
                            ) = &call_expr.arguments[0]
                            {
                                let ast::Property {
                                    base: _,
                                    key: _,
                                    separator: _,
                                    value,
                                    comma: _,
                                } = &object_expr.properties[0];
                                if let Some(
                                    ast::Expression::StringLit(
                                        ast::StringLit {
                                            base: _,
                                            value,
                                        },
                                    ),
                                ) = value
                                {
                                    self.bucket = value.clone()
                                }
                            }
                        }
                        "filter" => self
                            .filters
                            .push(FilterDecomposed::default()),
                        _ => {}
                    }
                }
            }
            ast::walk::Node::BinaryExpr(binary_expr) => {
                if binary_expr.operator
                    == ast::Operator::EqualOperator
                {
                    if let ast::Expression::Member(left) =
                        &binary_expr.left
                    {
                        if let ast::PropertyKey::Identifier(ident) =
                            &left.property
                        {
                            match ident.name.as_ref() {
                            "_measurement" => {
                                if let ast::Expression::StringLit(string_literal) = &binary_expr.right {
                                    if let Some(filter) = self.filters.last_mut() {
                                        filter.items.push(SchemaItem::Measurement(string_literal.value.clone()));
                                    }
                                }
                            },
                            "_field" => {
                                // This only matches when there is a single _field match.
                                if let ast::Expression::StringLit(string_literal) = &binary_expr.right {
                                    if let Some(filter) = self.filters.last_mut() {
                                        filter.items.push(SchemaItem::Field(string_literal.value.clone()));
                                    }
                                }
                            }
                            _ => {
                                // Treat these all as tag filters.
                                if let ast::Expression::StringLit(string_literal) = &binary_expr.right {
                                    if let Some(filter) = self.filters.last_mut() {
                                        filter.items.push(SchemaItem::TagValue(ident.name.clone(), string_literal.value.clone()));
                                    }
                                }
                            },
                        }
                        }
                    }
                }
            }
            ast::walk::Node::UnaryExpr(unary_expr) => {
                if unary_expr.operator
                    == ast::Operator::ExistsOperator
                {
                    if let ast::Expression::Member(member_expr) =
                        &unary_expr.argument
                    {
                        if let ast::PropertyKey::Identifier(
                            identifier,
                        ) = &member_expr.property
                        {
                            if let Some(filter) =
                                self.filters.last_mut()
                            {
                                filter.items.push(SchemaItem::Tag(
                                    identifier.name.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        true
    }
}

/// Find the composition statement.
///
/// The composition statement is identified as follows: a `from` function that contains
/// a yield with the name "_editor_composition".
#[derive(Default)]
struct CompositionStatementFinderVisitor {
    statement: Option<ast::ExprStmt>,
}

impl<'a> ast::walk::Visitor<'a>
    for CompositionStatementFinderVisitor
{
    fn visit(&mut self, node: ast::walk::Node<'a>) -> bool {
        if self.statement.is_some() {
            // If the statement was found, don't keep looking.
            return false;
        }

        if let ast::walk::Node::ExprStmt(expr_statement) = node {
            if let ast::Expression::PipeExpr(expr) =
                &expr_statement.expression
            {
                if let ast::Expression::Identifier(identifier) =
                    &expr.call.callee
                {
                    if identifier.name == "yield" {
                        expr.call.arguments.iter().any(|argument| {
                            if let ast::Expression::Object(object) = argument {
                                object.properties.iter().any(|property| {
                                    if let ast::PropertyKey::Identifier(key) = &property.key {
                                        if key.name == "name" {
                                            // Because we would have generated this statement, we'll always be able to
                                            // assert the simplicity of the yield name, i.e. it won't be a deeply nested
                                            // expression.
                                            if let Some(ast::Expression::StringLit(literal)) = &property.value {
                                                if literal.value == YIELD_IDENTIFIER {
                                                    self.statement = Some(expr_statement.clone());
                                                    return true;
                                                }
                                            }
                                        }
                                    }
                                    false
                                })
                            } else {
                                false
                            }
                        });
                        return false;
                    }
                }
            }
        }
        true
    }
}

type CompositionResult = Result<(), ()>;

/// Composition acts as the public entry point into the composition functionality.
struct Composition {
    file: ast::File,
}

impl ToString for Composition {
    fn to_string(&self) -> String {
        flux::formatter::convert_to_string(&self.file)
            .expect("Unable to convert composition file to string.")
    }
}

impl Composition {
    #[allow(dead_code)]
    fn new(file: ast::File) -> Self {
        Self { file }
    }

    /// Initialize an ast::File for use in composition.
    ///
    /// This must be called before any other composition can be made, as it'll set up the
    /// statement that will be managed by composition.
    #[allow(dead_code)]
    fn initialize(
        &mut self,
        bucket: &str,
        measurement: Option<&str>,
    ) -> CompositionResult {
        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&self.file),
        );

        let mut analyzer = CompositionQueryAnalyzer {
            bucket: bucket.to_string(),
            filters: vec![],
        };
        if let Some(m) = measurement {
            analyzer.filters.push(FilterDecomposed {
                items: vec![SchemaItem::Measurement(m.to_string())],
            });
        }

        let statement = analyzer.build();

        if let Some(expr_statement) = visitor.statement {
            self.remove_previous(expr_statement);
        }
        self.add_updated(statement);
        Ok(())
    }

    #[allow(dead_code)]
    fn add_measurement(
        &mut self,
        measurement: &str,
    ) -> CompositionResult {
        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&self.file),
        );
        if visitor.statement.is_none() {
            return Err(());
        }
        let expr_statement =
            visitor.statement.expect("Previous check failed.");

        let mut analyzer = CompositionQueryAnalyzer::default();
        analyzer.analyze(expr_statement.clone());
        if analyzer.filters.iter().any(|filter_decomposed| {
            matches!(
                &filter_decomposed.items[0],
                SchemaItem::Measurement(_)
            )
        }) {
            return Err(());
        } else {
            analyzer.filters.push(FilterDecomposed {
                items: vec![SchemaItem::Measurement(
                    measurement.to_string(),
                )],
            });
        }
        let statement = analyzer.build();

        self.remove_previous(expr_statement);
        self.add_updated(statement);
        Ok(())
    }

    #[allow(dead_code)]
    fn add_field(&mut self, field: &str) -> CompositionResult {
        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&self.file),
        );
        if visitor.statement.is_none() {
            return Err(());
        }
        let expr_statement =
            visitor.statement.expect("Previous check failed.");

        let mut analyzer = CompositionQueryAnalyzer::default();
        analyzer.analyze(expr_statement.clone());

        if let Some(idx) =
            analyzer.filters.iter().position(|filter_decomposed| {
                matches!(
                    &filter_decomposed.items[0],
                    SchemaItem::Field(_)
                )
            })
        {
            if analyzer.filters[idx].items.iter().any(|item| {
                match item {
                    SchemaItem::Field(f) => f.as_str() == field,
                    _ => false,
                }
            }) {
                return Err(());
            } else {
                analyzer.filters[idx]
                    .items
                    .push(SchemaItem::Field(field.to_string()));
            }
        } else {
            analyzer.filters.push(FilterDecomposed {
                items: vec![SchemaItem::Field(field.to_string())],
            });
        }

        let statement = analyzer.build();

        self.remove_previous(expr_statement);
        self.add_updated(statement);
        Ok(())
    }

    #[allow(dead_code)]
    fn add_tag(&mut self, tag: &str) -> CompositionResult {
        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&self.file),
        );
        if visitor.statement.is_none() {
            return Err(());
        }
        let expr_statement =
            visitor.statement.expect("Previous check failed.");

        let mut analyzer = CompositionQueryAnalyzer::default();
        analyzer.analyze(expr_statement.clone());

        analyzer.filters.push(FilterDecomposed {
            items: vec![SchemaItem::Tag(tag.to_string())],
        });

        let statement = analyzer.build();

        self.remove_previous(expr_statement);
        self.add_updated(statement);
        Ok(())
    }

    #[allow(dead_code)]
    fn add_tag_value(
        &mut self,
        tag_key: &str,
        tag_value: &str,
    ) -> CompositionResult {
        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&self.file),
        );
        if visitor.statement.is_none() {
            return Err(());
        }
        let expr_statement =
            visitor.statement.expect("Previous check failed.");

        let mut analyzer = CompositionQueryAnalyzer::default();
        analyzer.analyze(expr_statement.clone());

        analyzer.filters.push(FilterDecomposed {
            items: vec![SchemaItem::TagValue(
                tag_key.to_string(),
                tag_value.to_string(),
            )],
        });

        let statement = analyzer.build();

        self.remove_previous(expr_statement);
        self.add_updated(statement);
        Ok(())
    }

    fn remove_previous(&mut self, found_composition: ast::ExprStmt) {
        self.file.body = self
            .file
            .body
            .iter()
            .filter(|statement| match statement {
                ast::Statement::Expr(expression) => {
                    found_composition != *expression.as_ref()
                }
                _ => true,
            })
            .cloned()
            .collect();
    }

    fn add_updated(&mut self, statement: ast::PipeExpr) {
        self.file.body.insert(
            0,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    statement,
                )),
            })),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_strings(items: &[SchemaItem]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match &item {
                SchemaItem::Measurement(m) => Some(m.to_owned()),
                SchemaItem::Field(f) => Some(f.to_owned()),
                SchemaItem::Tag(t) => Some(t.to_owned()),
                SchemaItem::TagValue(_, _) => None,
            })
            .collect()
    }

    #[test]
    fn test_query_analyzer() {
        let fluxscript = r#"from(bucket: "an-composition")
|> range(start: v.timeRangeStart, stop: v.timeRangeStop)
|> filter(fn: (r) => r._measurement == "myMeasurement")
|> filter(fn: (r) => r._field == "myField" || r._field == "myOtherField")
|> filter(fn: (r) => exists r.anTag and exists r.anotherTag)
|> filter(fn: (r) => r.myTag == "anValue")
|> yield(name: "_editor_composition")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&ast),
        );
        let expr_statement =
            visitor.statement.expect("Previous check failed.");

        let mut analyzer = CompositionQueryAnalyzer::default();
        analyzer.analyze(expr_statement.clone());

        assert_eq!("an-composition".to_string(), analyzer.bucket);

        analyzer.filters.iter().for_each(|filter_decomposed| {
            match &filter_decomposed.items[0] {
                SchemaItem::Measurement(m) => {
                    assert_eq!(&"myMeasurement".to_string(), m)
                }
                SchemaItem::Field(_) => assert_eq!(
                    vec!["myField", "myOtherField"],
                    to_strings(&filter_decomposed.items)
                ),
                SchemaItem::Tag(_) => assert_eq!(
                    vec![
                        "anTag".to_string(),
                        "anotherTag".to_string()
                    ],
                    to_strings(&filter_decomposed.items)
                ),
                SchemaItem::TagValue(t, v) => assert_eq!(
                    ("myTag".to_string(), "anValue".to_string()),
                    (t.to_owned(), v.to_owned()),
                ),
            }
        });
    }

    /// Initializing composition for a file will add a composition-owned statement
    /// that will be the statement that filters will be added/removed.
    #[test]
    fn composition_initialize() {
        let fluxscript = r#"from(bucket: "my-bucket") |> yield(name: "my-result")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);

        composition.initialize(&"an-composition", None).unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> yield(name: "_editor_composition")
from(bucket: "my-bucket") |> yield(name: "my-result")
"#
            .to_string(),
            composition.to_string()
        );
    }

    /// Initializing composition on a file already initialized acts as a "reset",
    /// where buckets can be changed and filters removed.
    #[test]
    fn composition_initialize_reset() {
        let fluxscript = r#"from(bucket: "an-composition")
|> range(start: v.timeRangeStart, stop: v.timeRangeStop)
|> filter(fn: (r) => r.myTag == "myValue")
|> yield(name: "_editor_composition")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);

        composition.initialize(&"an-composition", None).unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        );
    }

    /// Initializing with a measurement will add a new |> filter call to go along
    /// with the rest of the function.
    #[test]
    fn composition_initialize_with_measurement() {
        let fluxscript = r#""#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);

        composition
            .initialize(&"an-composition", Some("myMeasurement"))
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        );
    }

    #[test]
    fn composition_add_measurement() {
        let fluxscript = r#""#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);

        composition.initialize(&"an-composition", None).unwrap();
        composition.add_measurement(&"myMeasurement").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_measurement_measurement_already_exists() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.

        assert!(composition
            .add_measurement(&"myMeasurement")
            .is_err());
    }

    #[test]
    fn composition_add_field() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_field(&"myField").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._field == "myField")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_field_with_measurement() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_field(&"myField").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r._field == "myField")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_field_field_already_exists() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r._field == "anField")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.

        assert!(composition.add_field(&"anField").is_err());
    }

    #[test]
    fn composition_add_field_second_field() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r._field == "firstField")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_field(&"secondField").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r._field == "firstField" or r._field == "secondField")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag_value() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_tag_value(&"tagKey", &"tagValue").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.tagKey == "tagValue")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag_value_second_tag_value() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r.tagKey1 == "tagValue1")
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_tag_value(&"tagKey2", &"tagValue2").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.tagKey1 == "tagValue1")
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r.tagKey2 == "tagValue2")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag_value_add_measurement_add_field_add_second_tag_value(
    ) {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_tag_value(&"tagKey1", &"tagValue1").unwrap();
        composition.add_measurement(&"anMeasurement").unwrap();
        composition.add_field(&"anField").unwrap();
        composition.add_tag_value(&"tagKey2", &"tagValue2").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.tagKey1 == "tagValue1")
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r._field == "anField")
    |> filter(fn: (r) => r.tagKey2 == "tagValue2")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_tag(&"tagKey").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => exists r.tagKey)
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag_second_tag() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => exists r.tagKey1)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r._field == "anField")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_tag(&"tagKey2").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => exists r.tagKey1)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r._field == "anField")
    |> filter(fn: (r) => exists r.tagKey2)
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag_tag_value_already_exists() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r.tagKey == "tagValue")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_tag(&"tagKey").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.tagKey == "tagValue")
    |> filter(fn: (r) => exists r.tagKey)
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag_add_measurement_add_field_add_second_tag()
    {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.add_tag(&"tagKey1").unwrap();
        composition.add_measurement(&"anMeasurement").unwrap();
        composition.add_field(&"anField").unwrap();
        composition.add_tag(&"tagKey2").unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => exists r.tagKey1)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r._field == "anField")
    |> filter(fn: (r) => exists r.tagKey2)
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }
}
