/// Composition functionality
///
/// This module covers all the functionality that comes from the Composition feature of the
/// LSP server. It's spec can be found in the docs/ folder of source control.
///
/// This module _only_ operates on an AST. It will never operate on semantic graph.
use std::collections::HashMap;

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

/// Analyze a query, understanding the various filters applied.
///
/// This struct is essentially a visitor, so it only provides a view into an existing
/// Composition statement, it does not make changes.
#[derive(Default)]
struct CompositionQueryAnalyzer {
    bucket: String,
    measurement: Option<String>,
    fields: Vec<String>,
    tag_values: HashMap<String, Vec<String>>, // [(TagName, [TagValue1s])]
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
        let mut inner = ast::Expression::PipeExpr(Box::new(pipe!(
            ast::Expression::Call(Box::new(from!(self
                .bucket
                .to_owned()))),
            range!()
        )));

        if let Some(measurement) = &self.measurement {
            inner = ast::Expression::PipeExpr(Box::new(pipe!(
                inner,
                filter!(
                    &["_measurement".to_string()],
                    &[measurement.to_owned()],
                    ast::LogicalOperator::OrOperator
                )
            )));
        }

        if !self.fields.is_empty() {
            inner = ast::Expression::PipeExpr(Box::new(pipe!(
                inner,
                filter!(
                    &vec!["_field".to_string(); self.fields.len()],
                    &self.fields,
                    ast::LogicalOperator::OrOperator
                )
            )));
        }

        if !self.tag_values.is_empty() {
            for (tag_key, tag_values) in self.tag_values.iter() {
                // XXX: Chunchun (10/24/22)
                // This is a work around for filter! signature
                let mut filter_keys =
                    vec!["".to_string(); tag_values.len()];
                let tag_keys: Vec<String> = filter_keys
                    .iter_mut()
                    .map(|_| tag_key.clone())
                    .collect();
                inner = ast::Expression::PipeExpr(Box::new(pipe!(
                    inner,
                    filter!(
                        tag_keys.as_slice(),
                        tag_values.as_ref(),
                        ast::LogicalOperator::OrOperator
                    )
                )));
            }
        }
        pipe!(inner, yield_!())
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
                    if identifier.name.as_str() == "from" {
                        if let ast::Expression::Object(object_expr) =
                            &call_expr.arguments[0]
                        {
                            let ast::Property {
                                base: _,
                                key: _,
                                separator: _,
                                value,
                                comma: _,
                            } = &object_expr.properties[0];
                            if let Some(ast::Expression::StringLit(
                                ast::StringLit { base: _, value },
                            )) = value
                            {
                                self.bucket = value.clone()
                            }
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
                                    match self.tag_values.get_mut(&ident.name) {
                                        Some(tag_value) => tag_value.push(string_literal.value.clone()),
                                        None => {
                                            self.tag_values.insert(ident.name.clone(), vec![string_literal.value.clone()]);
                                        }
                                    }
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
pub(crate) struct Composition {
    file: ast::File,
}

impl ToString for Composition {
    fn to_string(&self) -> String {
        flux::formatter::convert_to_string(&self.file)
            .expect("Unable to convert composition file to string.")
    }
}

impl Composition {
    pub(crate) fn new(file: ast::File) -> Self {
        Self { file }
    }

    pub(crate) fn composition_string(&self) -> Option<String> {
        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&self.file),
        );

        if let Some(expr_statement) = visitor.statement {
            let file = ast::File {
                base: ast::BaseNode::default(),
                metadata: "".into(),
                package: None,
                name: "".into(),
                imports: vec![],
                eof: vec![],
                body: vec![ast::Statement::Expr(Box::new(
                    expr_statement,
                ))],
            };
            match flux::formatter::convert_to_string(&file) {
                Ok(text) => return Some(text),
                _ => return None,
            }
        }
        None
    }

    /// Initialize an ast::File for use in composition.
    ///
    /// This must be called before any other composition can be made, as it'll set up the
    /// statement that will be managed by composition.
    pub(crate) fn initialize(
        &mut self,
        bucket: String,
        measurement: Option<String>,
        fields: Option<Vec<String>>,
        tag_values: Option<Vec<(String, String)>>,
    ) -> CompositionResult {
        let mut visitor =
            CompositionStatementFinderVisitor::default();
        flux::ast::walk::walk(
            &mut visitor,
            flux::ast::walk::Node::File(&self.file),
        );

        let mut analyzer = CompositionQueryAnalyzer {
            bucket,
            measurement,
            fields: fields.unwrap_or_default(),
            tag_values: if let Some(tag_values) = tag_values {
                let mut tags: HashMap<String, Vec<String>> =
                    HashMap::new();
                tag_values.iter().for_each(|(tag_key, tag_value)| {
                    match tags.get_mut(tag_key) {
                        Some(values) => {
                            values.push(tag_value.clone())
                        }
                        None => {
                            tags.insert(
                                tag_key.clone(),
                                vec![tag_value.clone()],
                            );
                        }
                    }
                });
                tags
            } else {
                HashMap::new()
            },
        };
        let statement = analyzer.build();

        if let Some(expr_statement) = visitor.statement {
            self.file.body = self
                .file
                .body
                .iter()
                .filter(|statement| match statement {
                    ast::Statement::Expr(expression) => {
                        expr_statement != *expression.as_ref()
                    }
                    _ => true,
                })
                .cloned()
                .collect();
        }

        self.file.body.insert(
            0,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    statement,
                )),
            })),
        );
        Ok(())
    }

    pub(crate) fn add_measurement(
        &mut self,
        measurement: String,
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
        if analyzer.measurement.is_some() {
            return Err(());
        } else {
            analyzer.measurement = Some(measurement)
        }
        let statement = analyzer.build();

        self.file.body = self
            .file
            .body
            .iter()
            .filter(|statement| match statement {
                ast::Statement::Expr(expression) => {
                    expr_statement != *expression.as_ref()
                }
                _ => true,
            })
            .cloned()
            .collect();

        self.file.body.insert(
            0,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    statement,
                )),
            })),
        );
        Ok(())
    }

    pub(crate) fn add_field(
        &mut self,
        field: String,
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

        if analyzer.fields.contains(&field) {
            return Err(());
        } else {
            analyzer.fields.push(field);
        }
        let statement = analyzer.build();

        self.file.body = self
            .file
            .body
            .iter()
            .filter(|statement| match statement {
                ast::Statement::Expr(expression) => {
                    expr_statement != *expression.as_ref()
                }
                _ => true,
            })
            .cloned()
            .collect();

        self.file.body.insert(
            0,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    statement,
                )),
            })),
        );

        Ok(())
    }

    pub(crate) fn remove_field(
        &mut self,
        field: String,
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

        let previous_len = analyzer.fields.len();
        analyzer.fields.retain(|f| f != &field);

        if previous_len == analyzer.fields.len() {
            return Err(());
        }
        let statement = analyzer.build();

        self.file.body = self
            .file
            .body
            .iter()
            .filter(|statement| match statement {
                ast::Statement::Expr(expression) => {
                    expr_statement != *expression.as_ref()
                }
                _ => true,
            })
            .cloned()
            .collect();

        self.file.body.insert(
            0,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    statement,
                )),
            })),
        );

        Ok(())
    }

    pub(crate) fn add_tag_value(
        &mut self,
        tag_key: String,
        tag_value: String,
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

        if analyzer.tag_values.contains_key(&tag_key)
            && analyzer.tag_values[&tag_key].contains(&tag_value)
        {
            return Err(());
        } else {
            match analyzer.tag_values.get_mut(&tag_key) {
                Some(tag_values) => {
                    tag_values.push(tag_value.clone())
                }
                None => {
                    analyzer.tag_values.insert(
                        tag_key.clone(),
                        vec![tag_value.clone()],
                    );
                }
            }
        }
        let statement = analyzer.build();

        self.file.body = self
            .file
            .body
            .iter()
            .filter(|statement| match statement {
                ast::Statement::Expr(expression) => {
                    expr_statement != *expression.as_ref()
                }
                _ => true,
            })
            .cloned()
            .collect();

        self.file.body.insert(
            0,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    statement,
                )),
            })),
        );

        Ok(())
    }

    pub(crate) fn remove_tag_value(
        &mut self,
        tag_key: String,
        tag_value: String,
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

        if !analyzer.tag_values.contains_key(&tag_key) {
            return Err(());
        }

        let previous_len: usize = analyzer.tag_values[&tag_key].len();

        match analyzer.tag_values.get_mut(&tag_key) {
            Some(tag_values) => {
                tag_values.retain(|value| value.ne(&tag_value));

                // remove the tag key if the tag values is an empty vec
                if tag_values.is_empty() {
                    analyzer.tag_values.remove(&tag_key);
                }
            }
            None => {
                return Err(());
            }
        }

        if analyzer.tag_values.contains_key(&tag_key)
            && previous_len == analyzer.tag_values[&tag_key].len()
        {
            return Err(());
        }

        let statement = analyzer.build();

        self.file.body = self
            .file
            .body
            .iter()
            .filter(|statement| match statement {
                ast::Statement::Expr(expression) => {
                    expr_statement != *expression.as_ref()
                }
                _ => true,
            })
            .cloned()
            .collect();

        self.file.body.insert(
            0,
            ast::Statement::Expr(Box::new(ast::ExprStmt {
                base: ast::BaseNode::default(),
                expression: ast::Expression::PipeExpr(Box::new(
                    statement,
                )),
            })),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_composition_string() {
        let fluxscript = r#"from(bucket: "an-composition")
|> yield(name: "_editor_composition")

from(bucket: "an-composition")
|> yield(name: "_another_id")
"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);
        let composition = Composition::new(ast);

        assert_eq!("from(bucket: \"an-composition\")\n    |> yield(name: \"_editor_composition\")\n".to_string(), composition.composition_string().unwrap());
    }

    #[test]
    fn test_composition_string_not_found() {
        let fluxscript = r#"from(bucket: "an-composition")
|> yield(name: "_not_a_composition")

from(bucket: "an-composition")
|> yield(name: "_another_id")
"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);
        let composition = Composition::new(ast);

        assert!(composition.composition_string().is_none());
    }

    #[test]
    fn test_composition_string_only_returns_composition() {
        let fluxscript = r#"from(bucket: "an-composition")
|> yield(name: "_editor_composition")

query1 = from(bucket: "an-composition")
"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);
        let composition = Composition::new(ast);

        assert_eq!("from(bucket: \"an-composition\")\n    |> yield(name: \"_editor_composition\")\n".to_string(), composition.composition_string().unwrap());
    }

    #[test]
    fn test_composition_string_will_return_updated_composition() {
        let fluxscript = r#"from(bucket: "an-composition")
|> yield(name: "_editor_composition")

query1 = from(bucket: "an-composition")
"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);
        let mut composition = Composition::new(ast);
        composition
            .add_measurement(String::from("myMeasurement"))
            .unwrap();

        assert_eq!("from(bucket: \"an-composition\")\n    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)\n    |> filter(fn: (r) => r._measurement == \"myMeasurement\")\n    |> yield(name: \"_editor_composition\")\n".to_string(), composition.composition_string().unwrap());
    }

    #[test]
    fn test_query_analyzer_analyze() {
        let fluxscript = r#"import "lib"

from(bucket: "an-composition")
|> range(start: v.timeRangeStart, stop: v.timeRangeStop)
|> filter(fn: (r) => r._measurement == "myMeasurement")
|> filter(fn: (r) => r._field == "myField" or r._field == "myOtherField")
|> filter(fn: (r) => exists r.anTag)
|> filter(fn: (r) => r.myTag == "anValue" or r.myTag == "anotherValue")
|> filter(fn: (r) => r.myOtherTag == "anotherValue")
|> filter(fn: (r) => exists r.anotherTag)
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

        assert_eq!(
            Some("myMeasurement".to_string()),
            analyzer.measurement
        );
        assert_eq!(vec!["myField", "myOtherField"], analyzer.fields);
        // let mut tags: HashMap<&str, Vec<str>>;
        assert_eq!(
            HashMap::from([
                (
                    "myTag".to_string(),
                    vec![
                        "anValue".to_string(),
                        "anotherValue".to_string(),
                    ],
                ),
                (
                    "myOtherTag".to_string(),
                    vec!["anotherValue".to_string()],
                ),
            ]),
            analyzer.tag_values
        );
    }

    /// Initializing composition for a file will add a composition-owned statement
    /// that will be the statement that filters will be added/removed.
    #[test]
    fn composition_initialize() {
        let fluxscript = r#"from(bucket: "my-bucket") |> yield(name: "my-result")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);

        composition
            .initialize(
                String::from("an-composition"),
                None,
                None,
                None,
            )
            .unwrap();

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

        composition
            .initialize(
                String::from("an-composition"),
                None,
                None,
                None,
            )
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        );
    }

    /// Initializing composition on a file which has import statements on first lines.
    #[test]
    fn composition_initialize_import_statement() {
        let fluxscript = r#"import "lib""#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);

        composition
            .initialize(
                String::from("an-composition"),
                None,
                None,
                None,
            )
            .unwrap();

        assert_eq!(
            r#"import "lib"

from(bucket: "an-composition")
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
            .initialize(
                String::from("an-composition"),
                Some(String::from("myMeasurement")),
                None,
                None,
            )
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

    /// Initializing composition with fields, tags, and tag values will populate the various
    /// `filter` calls and return the complete query.
    #[test]
    fn composition_initialize_with_extras() {
        let fluxscript = r#""#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);

        composition
            .initialize(
                String::from("an-composition"),
                Some("myMeasurement".into()),
                Some(vec!["myField".into(), "myField2".into()]),
                Some(vec![
                    ("myTag".into(), "myTagValue".into()),
                    ("myTag".into(), "myTagValue2".into()),
                ]),
            )
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> filter(fn: (r) => r._field == "myField" or r._field == "myField2")
    |> filter(fn: (r) => r.myTag == "myTagValue" or r.myTag == "myTagValue2")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        );
    }

    #[test]
    fn composition_add_measurement() {
        let fluxscript = r#"from(bucket: "an-composition") |> range(start: v.timeRangeStart, stop: v.timeRangeStop) |> yield(name: "_editor_composition")"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);
        let mut composition = Composition::new(ast);

        composition
            .add_measurement(String::from("myMeasurement"))
            .unwrap();

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
            .add_measurement(String::from("myMeasurement"))
            .is_err());
    }

    /// Modify existing composition, while retaining import statements on first lines.
    #[test]
    fn composition_add_measurement_import_statement() {
        let fluxscript = r#"import "lib"

from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> yield(name: "_editor_composition")
"#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);
        let mut composition = Composition::new(ast);

        composition
            .add_measurement(String::from("myMeasurement"))
            .unwrap();

        assert_eq!(
            r#"import "lib"

from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "myMeasurement")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        );
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
        composition.add_field(String::from("myField")).unwrap();

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
        composition.add_field(String::from("myField")).unwrap();

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

        assert!(composition
            .add_field(String::from("anField"))
            .is_err());
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
        composition.add_field(String::from("secondField")).unwrap();

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
    fn composition_remove_field() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r._field == "anField")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition.remove_field(String::from("anField")).unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_remove_field_multiple_fields() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r._field == "firstField" or r._field == "secondField" or r._field == "thirdField")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition
            .remove_field(String::from("secondField"))
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r._field == "firstField" or r._field == "thirdField")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_remove_field_field_not_exists() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r._field == "anField")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        assert!(composition
            .remove_field(String::from("anotherField"))
            .is_err());
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
        composition
            .add_tag_value(
                String::from("tagKey"),
                String::from("tagValue"),
            )
            .unwrap();

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
    fn composition_add_tag_value_tag_value_already_exists() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r.tagKey == "tagValue")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.

        assert!(composition
            .add_tag_value(
                String::from("tagKey"),
                String::from("tagValue")
            )
            .is_err());
    }

    #[test]
    fn composition_add_tag_value_new_tagset() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r.tagKey1 == "tagValue1")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition
            .add_tag_value(
                String::from("tagKey2"),
                String::from("tagValue2"),
            )
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r.tagKey1 == "tagValue1")
    |> filter(fn: (r) => r.tagKey2 == "tagValue2")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_add_tag_value_same_tag_key() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r.tagKey1 == "tagValue1")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition
            .add_tag_value(
                String::from("tagKey1"),
                String::from("tagValue2"),
            )
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r.tagKey1 == "tagValue1" or r.tagKey1 == "tagValue2")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_remove_tag_value() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r._field == "anField")
        |> filter(fn: (r) => r.tagKey == "tagValue")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition
            .remove_tag_value(
                String::from("tagKey"),
                String::from("tagValue"),
            )
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r._field == "anField")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_remove_tag_value_multiple_tag_values() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r.tagKey1 == "tagValue1")
        |> filter(fn: (r) => r.tagKey2 == "tagValue2")
        |> filter(fn: (r) => r.tagKey3 == "tagValue3")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        composition
            .remove_tag_value(
                String::from("tagKey2"),
                String::from("tagValue2"),
            )
            .unwrap();

        assert_eq!(
            r#"from(bucket: "an-composition")
    |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
    |> filter(fn: (r) => r._measurement == "anMeasurement")
    |> filter(fn: (r) => r.tagKey1 == "tagValue1")
    |> filter(fn: (r) => r.tagKey3 == "tagValue3")
    |> yield(name: "_editor_composition")
"#
            .to_string(),
            composition.to_string()
        )
    }

    #[test]
    fn composition_remove_tag_value_tag_value_not_exists() {
        let fluxscript = r#"from(bucket: "an-composition")
        |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
        |> filter(fn: (r) => r._measurement == "anMeasurement")
        |> filter(fn: (r) => r.tagKey1 == "tagValue1" and r.tagKey3 == "tagValue3")
        |> yield(name: "_editor_composition")
    "#;
        let ast = flux::parser::parse_string("".into(), &fluxscript);

        let mut composition = Composition::new(ast);
        // DON'T INITIALIZE THIS! WE'RE SIMULATING AN ALREADY INITIALIZED QUERY.
        assert!(composition
            .remove_tag_value(
                String::from("tagKey2"),
                String::from("tagValue2")
            )
            .is_err());
    }
}
