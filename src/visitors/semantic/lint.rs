use flux::semantic::walk::Node as WalkNode;
use lspower::lsp;

pub struct ExperimentalDiagnosticVisitor {
    namespaces: Vec<String>,
    pub diagnostics: Vec<lsp::Diagnostic>,
}

impl Default for ExperimentalDiagnosticVisitor {
    fn default() -> Self {
        Self {
            diagnostics: vec![],
            namespaces: vec!["experimental".into()],
        }
    }
}

impl<'a> flux::semantic::walk::Visitor<'a>
    for ExperimentalDiagnosticVisitor
{
    fn visit(&mut self, node: WalkNode<'a>) -> bool {
        if let WalkNode::Package(pkg) = node {
            // Is there an experimental import in this package? If not,
            // don't keep going. There's nothing to check here.
            let mut imports_experimental = false;
            pkg.files.iter().for_each(|file| {
                file.imports.iter().for_each(|import| {
                    if import.path.value.starts_with("experimental") {
                        imports_experimental = true;

                        if let Some(alias) = &import.alias {
                            self.namespaces
                                .push(format!("{}", alias.name));
                        } else {
                            let split: Vec<&str> = import
                                .path
                                .value
                                .split("/")
                                .collect();
                            if split.len() > 1 {
                                self.namespaces.push(
                                    split.last().unwrap().to_string(),
                                );
                            }
                        }
                    }
                })
            });
            return imports_experimental;
        }

        match node {
            WalkNode::CallExpr(expr) => {
                match &expr.callee {
                    flux::semantic::nodes::Expression::Identifier(id) => {
                        if self.namespaces.contains(&format!("{}", id.name)) {
                            self.diagnostics.push(lsp::Diagnostic {
                                range: lsp::Range {
                                    start: lsp::Position {
                                        line: expr.loc.start.line -1,
                                        character: expr.loc.start.column -1,
                                    },
                                    end: lsp::Position {
                                        line: expr.loc.end.line-1,
                                        character: expr.loc.end.column - 1,
                                    },
                                },
                                severity: Some(lsp::DiagnosticSeverity::HINT),
                                message: "experimental features can change often or be deleted/moved. Use with caution.".into(),
                                ..lsp::Diagnostic::default()
                            });
                        }
                    }
                    flux::semantic::nodes::Expression::Member(member) => {
                        if let flux::semantic::nodes::Expression::Identifier(id) = &member.object {
                            if self.namespaces.contains(&format!("{}", id.name)) {
                                self.diagnostics.push(lsp::Diagnostic {
                                    range: lsp::Range {
                                        start: lsp::Position {
                                            line: expr.loc.start.line -1,
                                            character: expr.loc.start.column -1,
                                        },
                                        end: lsp::Position {
                                            line: expr.loc.end.line-1,
                                            character: expr.loc.end.column - 1,
                                        },
                                    },
                                    severity: Some(lsp::DiagnosticSeverity::HINT),
                                    message: "experimental features can change often or be deleted/moved. Use with caution.".into(),
                                    ..lsp::Diagnostic::default()
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {},
        }
        true
    }
}
