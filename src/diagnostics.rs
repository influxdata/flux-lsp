/// Diagnostics for flux code
///
/// These diagnostics can range from informational lints to warnings and errors.
use lspower::lsp;

use flux::semantic::nodes::Package;

use super::visitors::semantic::ExperimentalDiagnosticVisitor;

/// Provide info about the nature of experimental.
///
/// While we want to encourage people to use the experimental package, we should
/// ensure they understand and are okay with the unstable nature of experimental.
/// The function can change or disappear at a moment's notice. If there isn't active
/// monitoring on the successful nature of queries using experimental, they may break
/// silently and cause headaches for consumers.
pub(crate) fn experimental_lint(
    pkg: &Package,
) -> Vec<lsp::Diagnostic> {
    let walker = flux::semantic::walk::Node::Package(pkg);
    let mut visitor = ExperimentalDiagnosticVisitor::default();

    flux::semantic::walk::walk(&mut visitor, walker);

    visitor.diagnostics
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn get_package(source: &str) -> flux::semantic::nodes::Package {
        let ast_pkg = flux::parser::parse_string("".into(), &source);
        let mut analyzer = flux::new_semantic_analyzer(
            flux::semantic::AnalyzerConfig::default(),
        )
        .unwrap();
        let (_, pkg) = analyzer.analyze_ast(&ast_pkg.into()).unwrap();
        pkg
    }

    #[test]
    fn experimental_lint_check() {
        let fluxscript = r#"import "experimental"
        
from(bucket: "my-bucket")
    |> range(start: -100d)
    |> filter(fn: (r) => r.value == "b")
    |> experimental.to(bucket: "out-bucket", org: "abc123", host: "https://myhost.example.com", token: "123abc")
"#;
        let package = get_package(&fluxscript);

        let diagnostics = experimental_lint(&package);

        assert_eq!(vec![lsp::Diagnostic {
            range: lsp::Range {
                start: lsp::Position {
                    line: 5, character: 7,
                },
                end : lsp::Position {
                    line: 5, character: 112,
                },
            },
            severity: Some(lsp::DiagnosticSeverity::HINT),
            message: "experimental features can change often or be deleted/moved. Use with caution.".into(),
            ..lsp::Diagnostic::default()
        }], diagnostics);
    }

    #[test]
    fn experimental_array_lint() {
        let fluxscript = r#"import "experimental/array"

array.concat(
    arr: [1,2],
    v: [3,4],
)
"#;
        let package = get_package(&fluxscript);

        let diagnostics = experimental_lint(&package);

        assert_eq!(vec![lsp::Diagnostic {
            range: lsp::Range {
                start: lsp::Position {
                    line: 2, character: 0,
                },
                end : lsp::Position {
                    line: 5, character: 1,
                },
            },
            severity: Some(lsp::DiagnosticSeverity::HINT),
            message: "experimental features can change often or be deleted/moved. Use with caution.".into(),
            ..lsp::Diagnostic::default()
        }], diagnostics);
    }
}
