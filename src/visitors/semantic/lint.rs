use flux::semantic::walk::Node as WalkNode;
use lspower::lsp;

pub struct ExperimentalDiagnosticVisitor {
    namespaces: Vec<String>,
    pub diagnostics: Vec<(Option<String>, lsp::Diagnostic)>,
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
        match node {
            WalkNode::Package(pkg) => {
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
                                    .split('/')
                                    .collect();
                                if split.len() > 1 {
                                    if let Some(namespace) = split.last() {
                                        self.namespaces.push(namespace.to_string());
                                    }
                                }
                            }
                        }
                    })
                });
                return imports_experimental;
            },
            WalkNode::CallExpr(expr) => {
                match &expr.callee {
                    flux::semantic::nodes::Expression::Identifier(id) => {
                        if self.namespaces.contains(&format!("{}", id.name)) {
                            self.diagnostics.push((expr.loc.file.clone(), lsp::Diagnostic {
                                range: expr.loc.clone().into(),
                                severity: Some(lsp::DiagnosticSeverity::HINT),
                                message: "experimental features can change often or be deleted/moved. Use with caution.".into(),
                                ..lsp::Diagnostic::default()
                            }));
                        }
                    }
                    flux::semantic::nodes::Expression::Member(member) => {
                        if let flux::semantic::nodes::Expression::Identifier(id) = &member.object {
                            if self.namespaces.contains(&format!("{}", id.name)) {
                                self.diagnostics.push((expr.loc.file.clone(), lsp::Diagnostic {
                                    range: expr.loc.clone().into(),
                                    severity: Some(lsp::DiagnosticSeverity::HINT),
                                    message: "experimental features can change often or be deleted/moved. Use with caution.".into(),
                                    ..lsp::Diagnostic::default()
                                }));
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
#[derive(Default)]
pub struct ContribDiagnosticVisitor {
    namespaces: Vec<String>,
    pub diagnostics: Vec<(Option<String>, lsp::Diagnostic)>,
}

impl<'a> flux::semantic::walk::Visitor<'a>
    for ContribDiagnosticVisitor
{
    fn visit(&mut self, node: WalkNode<'a>) -> bool {
        match node {
            WalkNode::Package(pkg) => {
                // Is there a contrib import in this package? If not,
                // don't keep going. There's nothing to check here.
                let mut imports_from_contrib = false;
                pkg.files.iter().for_each(|file| {
                    file.imports.iter().for_each(|import| {
                        if import.path.value.starts_with("contrib") {
                            imports_from_contrib = true;

                            if let Some(alias) = &import.alias {
                                self.namespaces
                                    .push(format!("{}", alias.name));
                            } else {
                                let split: Vec<&str> = import
                                    .path
                                    .value
                                    .split('/')
                                    .collect();
                                if split.len() > 1 {
                                    if let Some(namespace) = split.last()
                                    {
                                        self.namespaces
                                            .push(namespace.to_string());
                                    }
                                }
                            }
                        }
                    })
                });
                return imports_from_contrib;
            },
            WalkNode::CallExpr(expr) => {
                match &expr.callee {
                    flux::semantic::nodes::Expression::Identifier(id) => {
                        if self.namespaces.contains(&format!("{}", id.name)) {
                            self.diagnostics.push((id.loc.file.clone(), lsp::Diagnostic {
                                range: expr.loc.clone().into(),
                                severity: Some(lsp::DiagnosticSeverity::HINT),
                                message: "contrib packages are user-contributed, and do not carry with them the same compatibility guarantees as the standard library. Use with caution.".into(),
                                ..lsp::Diagnostic::default()
                            }));
                        }
                    }
                    flux::semantic::nodes::Expression::Member(member) => {
                        if let flux::semantic::nodes::Expression::Identifier(id) = &member.object {
                            if self.namespaces.contains(&id.name.to_string()) {
                                self.diagnostics.push((id.loc.file.clone(), lsp::Diagnostic {
                                    range: expr.loc.clone().into(),
                                    severity: Some(lsp::DiagnosticSeverity::HINT),
                                    message: "contrib packages are user-contributed, and do not carry with them the same compatibility guarantees as the standard library. Use with caution.".into(),
                                    ..lsp::Diagnostic::default()
                                }));
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

pub struct InfluxDBIdentifierDiagnosticVisitor {
    names: Vec<String>,
    pub diagnostics: Vec<(Option<String>, lsp::Diagnostic)>,
}

impl Default for InfluxDBIdentifierDiagnosticVisitor {
    fn default() -> Self {
        Self {
            diagnostics: vec![],
            names: vec!["v".into(), "task".into(), "params".into()],
        }
    }
}

impl<'a> flux::semantic::walk::Visitor<'a>
    for InfluxDBIdentifierDiagnosticVisitor
{
    fn visit(&mut self, node: WalkNode<'a>) -> bool {
        if let WalkNode::VariableAssgn(assign) = node {
            if self.names.contains(&assign.id.name.to_string()) {
                self.diagnostics.push((assign.loc.file.clone(), lsp::Diagnostic {
                    range: assign.id.loc.clone().into(),
                    severity: Some(lsp::DiagnosticSeverity::WARNING),
                    message: format!("Avoid using `{}` as an identifier name. In some InfluxDB contexts, it may be provided at runtime.", assign.id.name),
                    ..lsp::Diagnostic::default()
                }));
            }
        }
        true
    }
}
