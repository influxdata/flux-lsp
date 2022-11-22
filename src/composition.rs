/// Composition functionality
///
/// This module covers all the functionality that comes from the Composition feature of the
/// LSP server. It's spec can be found in the docs/ folder of source control.
///
/// This module _only_ operates on an AST. It will never operate on semantic graph.
use flux::ast;
use itertools::Itertools;

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
    ($arguments:expr) => {
        ast::CallExpr {
            arguments: vec![ast::Expression::Object(Box::new(
                $arguments,
            ))],
            base: ast::BaseNode::default(),
            callee: ast::Expression::Identifier(ast::Identifier {
                base: ast::BaseNode::default(),
                name: "range".into(),
            }),
            lparen: vec![],
            rparen: vec![],
        }
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
    ($key:expr, $values:expr, $operator:expr) => {
        filter!($key, $values, $operator, chained_binary_eq_expr($operator, $key, $values).expect("chained_binary_eq_expr failed"))
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

macro_rules! pipe {
    ($a:expr, $b:expr) => {
        ast::PipeExpr {
            argument: $a,
            base: ast::BaseNode::default(),
            call: $b,
        }
    };
}

/// Analyze a query, understanding the various filters applied.
///
/// This struct is essentially a visitor, so it only provides a view into an existing
/// Composition statement, it does not make changes.
#[derive(Clone, Default)]
struct CompositionStatementAnalyzer {
    bucket: String,
    measurement: Option<String>,
    fields: Vec<String>,
    tag_values: Vec<(String, String)>, // (TagName, TagValue)
    calls: Vec<ast::CallExpr>,
    range_arguments: Option<ast::ObjectExpr>,
}

impl CompositionStatementAnalyzer {
    fn analyze(statement: ast::ExprStmt) -> Self {
        let mut analyzer = Self::default();
        ast::walk::walk(
            &mut analyzer,
            flux::ast::walk::Node::from_stmt(&ast::Statement::Expr(
                Box::new(statement),
            )),
        );
        analyzer
    }

    fn build(&mut self) -> ast::PipeExpr {
        let range_arguments = if let Some(arguments) =
            &self.range_arguments
        {
            arguments.clone()
        } else {
            ast::ObjectExpr {
                base: ast::BaseNode::default(),
                properties: vec![
                    ast::Property {
                        base: ast::BaseNode::default(),
                        key: ast::PropertyKey::Identifier(
                            ast::Identifier {
                                base: ast::BaseNode::default(),
                                name: "start".into(),
                            },
                        ),
                        value: Some(ast::Expression::Member(
                            Box::new(ast::MemberExpr {
                                base: ast::BaseNode::default(),
                                lbrack: vec![],
                                rbrack: vec![],
                                object: ast::Expression::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "v".into(),
                                    },
                                ),
                                property:
                                    ast::PropertyKey::Identifier(
                                        ast::Identifier {
                                            base:
                                                ast::BaseNode::default(
                                                ),
                                            name: "timeRangeStart"
                                                .into(),
                                        },
                                    ),
                            }),
                        )),
                        comma: vec![],
                        separator: vec![],
                    },
                    ast::Property {
                        base: ast::BaseNode::default(),
                        key: ast::PropertyKey::Identifier(
                            ast::Identifier {
                                base: ast::BaseNode::default(),
                                name: "stop".into(),
                            },
                        ),
                        value: Some(ast::Expression::Member(
                            Box::new(ast::MemberExpr {
                                base: ast::BaseNode::default(),
                                lbrack: vec![],
                                rbrack: vec![],
                                object: ast::Expression::Identifier(
                                    ast::Identifier {
                                        base: ast::BaseNode::default(
                                        ),
                                        name: "v".into(),
                                    },
                                ),
                                property:
                                    ast::PropertyKey::Identifier(
                                        ast::Identifier {
                                            base:
                                                ast::BaseNode::default(
                                                ),
                                            name: "timeRangeStop"
                                                .into(),
                                        },
                                    ),
                            }),
                        )),
                        comma: vec![],
                        separator: vec![],
                    },
                ],
                lbrace: vec![],
                rbrace: vec![],
                with: None,
            }
        };
        let mut inner = pipe!(
            ast::Expression::Call(Box::new(from!(self
                .bucket
                .to_owned()))),
            range!(range_arguments)
        );

        if let Some(measurement) = &self.measurement {
            inner = pipe!(
                ast::Expression::PipeExpr(Box::new(inner)),
                filter!(
                    &["_measurement".to_string()],
                    &[measurement.to_owned()],
                    ast::LogicalOperator::OrOperator
                )
            );
        }

        if !self.fields.is_empty() {
            inner = pipe!(
                ast::Expression::PipeExpr(Box::new(inner)),
                filter!(
                    &vec!["_field".to_string(); self.fields.len()],
                    &self.fields,
                    ast::LogicalOperator::OrOperator
                )
            );
        }

        if !self.tag_values.is_empty() {
            let tags = self
                .tag_values
                .iter()
                .map(|(key, _value)| key)
                .unique()
                .collect::<Vec<&String>>();
            tags.iter().for_each(|tag| {
                let tag = tag.to_string();
                let values = self
                    .tag_values
                    .iter()
                    .filter(|(key, _value)| key == &tag)
                    .map(|(_key, value)| value.to_string())
                    .collect::<Vec<String>>();
                inner = pipe!(
                    ast::Expression::PipeExpr(Box::new(
                        inner.clone()
                    )),
                    filter!(
                        vec![tag; values.len()].as_slice(),
                        values.as_ref(),
                        ast::LogicalOperator::OrOperator
                    )
                );
            });
        }
        for call_expression in self.calls.iter() {
            inner = pipe!(
                ast::Expression::PipeExpr(Box::new(inner)),
                call_expression.clone()
            );
        }

        inner
    }
}

impl<'a> ast::walk::Visitor<'a> for CompositionStatementAnalyzer {
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
                            return false;
                        }
                        "range" => {
                            self.range_arguments = call_expr.arguments.last().cloned().map(|expression| match expression {
                                ast::Expression::Object(object) => *object,
                                _ => unreachable!("CallExpr arguments can only be Expression::Objecs"),
                            });
                            return false;
                        }
                        "filter" => {
                            // In the case of _any_ other function calls, we can no longer expect
                            // a filter call to be related to schema. It _might_ still be, but after
                            // other calls, we have no guarantee.
                            if !self.calls.is_empty() {
                                self.calls.push(call_expr.clone());
                                return false;
                            }
                            // We rely on the logic below for checking the binary expressions in the
                            // filter to get schema data.
                            return true;
                        }
                        _ => {
                            self.calls.push(call_expr.clone());
                            return false;
                        }
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
                                    self.measurement = Some(string_literal.value.clone());
                                }
                            },
                            "_field" => {
                                // This only matches when there is a single _field match.
                                if let ast::Expression::StringLit(string_literal) = &binary_expr.right {
                                    self.fields.push(string_literal.value.clone());
                                }
                            }
                            _ => {
                                // Treat these all as tag filters.
                                if let ast::Expression::StringLit(string_literal) = &binary_expr.right {
                                    self.tag_values.push((ident.name.clone(), string_literal.value.clone()));
                                }
                            },
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

impl PartialEq for CompositionStatementAnalyzer {
    fn eq(&self, other: &Self) -> bool {
        // An analyzer is considered equal if the schema filters applied are the same. The calls made after that
        // are not (currently) considered.
        self.bucket == other.bucket
            && self.measurement == other.measurement
            && self.fields == other.fields
            && self.tag_values == other.tag_values
    }
}
impl Eq for CompositionStatementAnalyzer {}

type CompositionResult = Result<(), ()>;

/// Composition acts as the public entry point into the composition functionality.
#[derive(Clone)]
pub(crate) struct Composition {
    file: ast::File,

    statement_index: usize,
    analyzer: CompositionStatementAnalyzer,
}

impl ToString for Composition {
    fn to_string(&self) -> String {
        flux::formatter::convert_to_string(&self.file)
            .expect("Unable to convert composition file to string.")
    }
}

impl Composition {
    pub(crate) fn new(
        mut file: ast::File,
        bucket: String,
        measurement: Option<String>,
        fields: Vec<String>,
        tag_values: Vec<(String, String)>,
    ) -> Self {
        let mut analyzer = CompositionStatementAnalyzer {
            bucket,
            measurement,
            fields,
            tag_values,
            ..Default::default()
        };

        // Find the index of the first expression statement in the ast file. The composition
        // will be inserted before that first expression statement.
        let statement_index = file
            .body
            .iter()
            .position(|statement| {
                matches!(statement, ast::Statement::Expr(_))
            })
            .unwrap_or(file.body.len());

        file.body.insert(
            statement_index,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    analyzer.build(),
                )),
            })),
        );

        Self {
            file,
            analyzer,
            statement_index,
        }
    }

    /// Sync the composition statement with the analyzer.
    // This is, for obvious reasons, inefficient. It moves the items in the vec
    // around a lot. Premature optimization and blah blah blah.
    fn sync(&mut self) {
        self.file.body.remove(self.statement_index);
        self.file.body.insert(
            self.statement_index,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    self.analyzer.build(),
                )),
            })),
        );
    }

    /// Resolve composition with a new ast File.
    ///
    /// This is the logic that re-attaches to the new composition. The logic
    /// is complex, but the core of the work is the analyzer, which checks
    /// all the statements to find the matching one.
    ///
    /// In the event the composition can't re-attach to the new AST, usually
    /// through ambiguity, an error is returned. The Composition should then
    /// be discarded.
    pub fn resolve_with_ast(
        &mut self,
        file: ast::File,
    ) -> Result<(), ()> {
        self.file = file;
        let matches = self
            .file
            .body
            .iter()
            .enumerate()
            .filter(|(_index, statement)| {
                matches!(statement, ast::Statement::Expr(_))
            })
            .map(|(index, statement)| {
                let expression = match statement {
                    ast::Statement::Expr(expr) => expr,
                    _ => unreachable!("Previous filter failed"),
                };
                let analyzer = CompositionStatementAnalyzer::analyze(
                    *expression.clone(),
                );
                (index, analyzer)
            })
            .filter(|(_index, analyzer)| {
                analyzer.clone() == self.analyzer
            })
            .collect::<Vec<(usize, CompositionStatementAnalyzer)>>();
        if matches.len() > 1 {
            log::error!(
                "Too many matches for composition statement."
            );
            Err(())
        } else {
            match matches.last() {
                Some((index, analyzer)) => {
                    self.statement_index = *index;
                    self.analyzer = analyzer.clone();
                    Ok(())
                }
                None => {
                    log::error!("Could not find matching composition statement.");
                    Err(())
                }
            }
        }
    }

    pub(crate) fn set_measurement(
        &mut self,
        measurement: String,
    ) -> CompositionResult {
        self.analyzer.measurement = Some(measurement);
        self.sync();
        Ok(())
    }

    pub(crate) fn add_field(
        &mut self,
        field: String,
    ) -> CompositionResult {
        if self.analyzer.fields.contains(&field) {
            return Err(());
        } else {
            self.analyzer.fields.push(field);
        }
        self.sync();
        Ok(())
    }

    pub(crate) fn remove_field(
        &mut self,
        field: String,
    ) -> CompositionResult {
        if self.analyzer.fields.contains(&field) {
            self.analyzer.fields.retain(|f| f != &field);
        } else {
            return Err(());
        }
        self.sync();
        Ok(())
    }

    pub(crate) fn add_tag_value(
        &mut self,
        tag_key: String,
        tag_value: String,
    ) -> CompositionResult {
        let tag_pair = (tag_key, tag_value);
        if self.analyzer.tag_values.contains(&tag_pair) {
            return Err(());
        } else {
            self.analyzer.tag_values.push(tag_pair);
        }
        self.sync();
        Ok(())
    }

    pub(crate) fn remove_tag_value(
        &mut self,
        tag_key: String,
        tag_value: String,
    ) -> CompositionResult {
        let tag_pair = (tag_key, tag_value);
        if self.analyzer.tag_values.contains(&tag_pair) {
            self.analyzer.tag_values.retain(|p| !p.eq(&tag_pair));
        } else {
            return Err(());
        }
        self.sync();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Initializing a composition will create a statement in the flux ast file
    /// that is managed by the composition.
    #[test]
    fn test_composition() {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(fn: (r) => r._field == "myField1" or r._field == "myField2")
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// Composition statements are added before the first expression statement, but after
    /// any other statement types.
    #[test]
    fn test_composition_with_content() {
        let fluxscript = r#"import "experimental"

option task = {
    name: "myTask",
    every: 1h,
    offset: 10m,
    cron: "0 2 * * *",
}

myVar = 83

from(bucket: "myExperimentalBucket")
  |> range(start: -12m)
  |> filter(fn: (r) => r._measurement == "anMeasurement")
  |> pivot()
  |> experimental.to(bucket: "myExperimentalBucketPivot")
"#
        .to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let expected = r#"import "experimental"

option task = {name: "myTask", every: 1h, offset: 10m, cron: "0 2 * * *"}

myVar = 83

from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(fn: (r) => r._field == "myField1" or r._field == "myField2")
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")

from(bucket: "myExperimentalBucket")
    |> range(start: -12m)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> pivot()
    |> experimental.to(bucket: "myExperimentalBucketPivot")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// If there are only non-expression statements, add the composition to the end.
    #[test]
    fn test_composition_with_only_imports_and_variables() {
        let fluxscript = r#"import "experimental"

option task = {
    name: "myTask",
    every: 1h,
    offset: 10m,
    cron: "0 2 * * *",
}

myVar = 83
"#
        .to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let expected = r#"import "experimental"

option task = {name: "myTask", every: 1h, offset: 10m, cron: "0 2 * * *"}

myVar = 83

from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(fn: (r) => r._field == "myField1" or r._field == "myField2")
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// Any filter calls that are after a call that are not one
    /// of from/range/filter cannot be used to determine schema requirements,
    /// as the columns referenced may have originated in a previous call.
    #[test]
    fn test_composition_with_filters_after_other_calls() {
        let ast = flux::parser::parse_string("".into(), &"");
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec![],
            vec![],
        );

        let fluxscript = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> map(fn: (r) => ({r with myColumn: r._value / 100}))
    |> filter(fn: (r) => r.myOtherColumn == "percent")
"#
        .to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        assert!(composition.resolve_with_ast(ast).is_ok());
        let expected: Vec<(String, String)> = vec![];
        assert_eq!(expected, composition.analyzer.tag_values)
    }

    /// When a non-expression statement is added above the composition statement, the
    /// Composition struct is able to find its statement in the new ast and update
    /// it appropriately.
    #[test]
    fn test_composition_resolve_with_ast_preceding_var() {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"myVar = 83

{}"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());

        assert_eq!(1, composition.statement_index);
        assert!(composition
            .add_field("myField3".to_string())
            .is_ok());

        let expected = r#"myVar = 83

from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(
        fn: (r) => r._field == "myField1" or (r._field == "myField2" or r._field == "myField3"),
    )
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// A user can change the composition range and that range is retained
    /// in the composition.
    #[test]
    fn test_composition_resolve_with_ast_range_change() {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec![],
            vec![],
        );
        let fluxscript = r#"from(bucket: "myBucket")
    |> range(start: -24h)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
"#;
        let new_ast =
            flux::parser::parse_string("".into(), &fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());
        assert!(composition.add_field("myField".into()).is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: -24h)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(fn: (r) => r._field == "myField")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// When an expression statement is added above the composition statement, the
    /// Composition struct is able to find its statement in the new ast and update
    /// it appropriately.
    #[test]
    fn test_composition_resolve_with_ast_preceding_expr() {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"from(bucket: "anBucket")
|> range(start: -12m)

{}"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());

        assert_eq!(1, composition.statement_index);
        assert!(composition
            .add_field("myField3".to_string())
            .is_ok());

        let expected = r#"from(bucket: "anBucket")
    |> range(start: -12m)
from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(
        fn: (r) => r._field == "myField1" or (r._field == "myField2" or r._field == "myField3"),
    )
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// When an expression statement is added above the composition statement, and the
    /// new statement matches buckets, the composition can still find its statement
    /// in the file.
    #[test]
    fn test_composition_resolve_with_ast_preceding_expr_matching_bucket(
    ) {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"from(bucket: "myBucket")
|> range(start: -12m)

{}"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());

        assert_eq!(1, composition.statement_index);
        assert!(composition
            .add_field("myField3".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: -12m)
from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(
        fn: (r) => r._field == "myField1" or (r._field == "myField2" or r._field == "myField3"),
    )
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// When an expression statement is added above the composition statement, and the
    /// new statement matches bucket and measurement, the composition can still find
    /// its statement in the file.
    #[test]
    fn test_composition_resolve_with_ast_preceding_expr_matching_bucket_and_measurement(
    ) {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"from(bucket: "myBucket")
|> range(start: -12m)
|> filter(fn: (r) => r._measurement == "myMeasurement")

{}"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());

        assert_eq!(1, composition.statement_index);
        assert!(composition
            .add_field("myField3".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: -12m)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(
        fn: (r) => r._field == "myField1" or (r._field == "myField2" or r._field == "myField3"),
    )
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// When an expression statement is added above the composition statement, and the
    /// new statement matches bucket and measurement, the composition can still find
    /// its statement in the file.
    #[test]
    fn test_composition_resolve_with_ast_preceding_expr_matching_bucket_and_measurement_and_fields(
    ) {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"from(bucket: "myBucket")
|> range(start: -12m)
|> filter(fn: (r) => r._measurement == "myMeasurement")
|> filter(fn: (r) => r._field == "myField1" or r._field == "myField2")

{}"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());

        assert_eq!(1, composition.statement_index);
        assert!(composition
            .add_field("myField3".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: -12m)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(fn: (r) => r._field == "myField1" or r._field == "myField2")
from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(
        fn: (r) => r._field == "myField1" or (r._field == "myField2" or r._field == "myField3"),
    )
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// When there are two statements that could both match the composition
    /// statement, an error occurs.
    #[test]
    fn test_composition_resolve_with_ast_preceding_expr_matching_all()
    {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"from(bucket: "myBucket")
|> range(start: -12m)
|> filter(fn: (r) => r._measurement == "myMeasurement")
|> filter(fn: (r) => r._field == "myField1" or r._field == "myField2")
|> filter(fn: (r) => r.myTagKey == "myTagValue" and r.myTagKey2 == "myTagValue2")

{}"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_err());
    }

    /// When an expression statement is added after the composition statement, the
    /// Composition struct is able to find its statement in the new ast and update
    /// it appropriately.
    #[test]
    fn test_composition_resolve_with_ast_following_expr() {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"{}

from(bucket: "anBucket")
    |> range(start: -12m)
"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());

        assert_eq!(0, composition.statement_index);
        assert!(composition
            .add_field("myField3".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(
        fn: (r) => r._field == "myField1" or (r._field == "myField2" or r._field == "myField3"),
    )
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")

from(bucket: "anBucket")
    |> range(start: -12m)
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// When the query is modified to add new calls after the filter calls, those new calls
    /// are preserved after a composition change.
    #[test]
    fn test_composition_resolve_with_ast_add_calls() {
        let fluxscript = "".to_string();
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec!["myField1".into(), "myField2".into()],
            vec![
                ("myTagKey".into(), "myTagValue".into()),
                ("myTagKey2".into(), "myTagValue2".into()),
            ],
        );

        let new_fluxscript = format!(
            r#"{}
|> group(columns: ["myNonexistentColumn"])
|> sort()
"#,
            composition.to_string()
        );
        let new_ast =
            flux::parser::parse_string("".into(), &new_fluxscript);
        assert!(composition.resolve_with_ast(new_ast).is_ok());

        assert_eq!(0, composition.statement_index);
        assert!(composition
            .add_field("myField3".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(
        fn: (r) => r._field == "myField1" or (r._field == "myField2" or r._field == "myField3"),
    )
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
    |> group(columns: ["myNonexistentColumn"])
    |> sort()
"#.to_string();
        assert_eq!(expected, composition.to_string());
    }

    /// A measurement filter can be added to a composition statement.
    #[test]
    fn test_composition_set_measurement() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![],
        );

        assert!(composition
            .set_measurement("myMeasurement".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// Only one measurement can be added at one time. An error occurs if
    /// the measurement has already been set.
    #[test]
    fn test_composition_set_measurement_already_set() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            Some("myMeasurement".into()),
            vec![],
            vec![],
        );

        assert!(composition
            .set_measurement("anotherMeasurement".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anotherMeasurement")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// A field filter with multiple fields can be added.
    #[test]
    fn test_composition_add_field() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![],
        );

        assert!(composition.add_field("myField".to_string()).is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._field == "myField")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// When multiple fields are added to the filter, the field values are OR'd
    /// together.
    #[test]
    fn test_composition_add_fields() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec!["myField".into()],
            vec![],
        );

        assert!(composition
            .add_field("myField2".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._field == "myField" or r._field == "myField2")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// Adding a field that is already part of the composition results in an error.
    #[test]
    fn test_composition_add_field_already_added() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec!["myField".into()],
            vec![],
        );

        assert!(composition
            .add_field("myField".to_string())
            .is_err());
    }

    /// Fields are removed from the composition.
    #[test]
    fn test_composition_remove_field() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec!["myField".into()],
            vec![],
        );

        assert!(composition
            .remove_field("myField".to_string())
            .is_ok());

        let expected = r#"from(bucket: "myBucket") |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// Removing a field that hasn't been added to the composition results
    /// in an error.
    #[test]
    fn test_composition_remove_field_not_added() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec!["myField".into()],
            vec![],
        );

        assert!(composition
            .remove_field("myField2".to_string())
            .is_err());
    }

    /// Filters for tag keys/values can be added to the composition.
    #[test]
    fn test_add_tag_value() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![],
        );

        assert!(composition
            .add_tag_value(
                "myTagKey".to_string(),
                "myTagValue".to_string()
            )
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// Tag filters are their own individual filter calls.
    #[test]
    fn test_add_tag_values() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![("myTagKey".into(), "myTagValue".into())],
        );

        assert!(composition
            .add_tag_value(
                "myTagKey2".to_string(),
                "myTagValue2".to_string()
            )
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.myTagKey == "myTagValue")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// Tags filters with multiple tag values result in a single
    /// `filter` call with OR operators.
    #[test]
    fn test_add_tag_values_with_same_key() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![("myTagKey".into(), "myTagValue".into())],
        );

        assert!(composition
            .add_tag_value(
                "myTagKey".to_string(),
                "myTagValue3".to_string()
            )
            .is_ok());
        assert!(composition
            .add_tag_value(
                "myTagKey2".to_string(),
                "myTagValue2".to_string()
            )
            .is_ok());

        let expected = r#"from(bucket: "myBucket")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r.myTagKey == "myTagValue" or r.myTagKey == "myTagValue3")
    |> filter(fn: (r) => r.myTagKey2 == "myTagValue2")
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// Adding a tag key/value that already exists in the composition results
    /// in an error.
    #[test]
    fn test_add_tag_value_already_added() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![("myTagKey".into(), "myTagValue".into())],
        );

        assert!(composition
            .add_tag_value(
                "myTagKey".to_string(),
                "myTagValue".to_string()
            )
            .is_err());
    }

    /// A tag key/value pair can be removed from the composition.
    #[test]
    fn test_remove_tag_value() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![("myTagKey".into(), "myTagValue".into())],
        );

        assert!(composition
            .remove_tag_value(
                "myTagKey".to_string(),
                "myTagValue".to_string()
            )
            .is_ok());

        let expected = r#"from(bucket: "myBucket") |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
"#;
        assert_eq!(expected, composition.to_string());
    }

    /// Attempting to remove a tag key/value pair that wasn't added to the composition
    /// results in an error.
    #[test]
    fn test_remove_tag_value_not_added() {
        let ast =
            flux::parser::parse_string("".into(), &"".to_string());
        let mut composition = Composition::new(
            ast,
            "myBucket".into(),
            None,
            vec![],
            vec![],
        );

        assert!(composition
            .remove_tag_value(
                "myTagKey".to_string(),
                "myTagValue".to_string()
            )
            .is_err());
    }
}
