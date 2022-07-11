/// Diagnostics for flux code
///
/// These diagnostics can range from informational lints to warnings and errors.
use lspower::lsp;

use flux::semantic::nodes::Package;

use super::visitors::semantic::{
    ContribDiagnosticVisitor, ExperimentalDiagnosticVisitor,
    InfluxDBIdentifierDiagnosticVisitor,
};

/// Provide info about the nature of experimental.
///
/// While we want to encourage people to use the experimental package, we should
/// ensure they understand and are okay with the unstable nature of experimental.
/// The function can change or disappear at a moment's notice. If there isn't active
/// monitoring on the successful nature of queries using experimental, they may break
/// silently and cause headaches for consumers.
pub(crate) fn experimental_lint(
    pkg: &Package,
) -> Vec<(Option<String>, lsp::Diagnostic)> {
    let visitor = crate::walk_semantic_package!(
        ExperimentalDiagnosticVisitor::default(),
        pkg
    );
    visitor.diagnostics
}

/// Provide info about the nature of contrib.
///
/// The packages in contrib are provided by individual users, and don't carry the
/// support or compatibility guarantees that the stdlib usually carries. These
/// functions should be used with caution.
pub(crate) fn contrib_lint(
    pkg: &Package,
) -> Vec<(Option<String>, lsp::Diagnostic)> {
    let visitor = crate::walk_semantic_package!(
        ContribDiagnosticVisitor::default(),
        pkg
    );
    visitor.diagnostics
}

pub(crate) fn no_influxdb_identifiers(
    pkg: &Package,
) -> Vec<(Option<String>, lsp::Diagnostic)> {
    let visitor = crate::walk_semantic_package!(
        InfluxDBIdentifierDiagnosticVisitor::default(),
        pkg
    );
    visitor.diagnostics
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn get_package(source: &str) -> flux::semantic::nodes::Package {
        let ast_pkg =
            flux::parser::parse_string("script.flux".into(), &source);
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

        assert_eq!(vec![(Some("script.flux".to_string()), lsp::Diagnostic {
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
        })], diagnostics);
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

        assert_eq!(vec![(Some("script.flux".to_string()), lsp::Diagnostic {
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
        })], diagnostics);
    }

    #[test]
    fn contrib_lint_check() {
        let fluxscript = r#"import "contrib/jsternberg/influxdb"

influxdb.select(
    from: "example-bucket",
    start: -1d,
    stop: now(),
    m: "example-measurement",
    fields: [],
    where: (r) => true,
    host: "https://example.com",
    org: "example-org",
    token: "MySuP3rSecr3Tt0k3n",
)
"#;
        let package = get_package(&fluxscript);

        let diagnostics = contrib_lint(&package);

        assert_eq!(vec![(Some("script.flux".to_string()), lsp::Diagnostic {
            range: lsp::Range {
                start: lsp::Position {
                    line: 2, character: 0,
                },
                end : lsp::Position {
                    line: 12, character: 1,
                },
            },
            severity: Some(lsp::DiagnosticSeverity::HINT),
            message: "contrib packages are user-contributed, and do not carry with them the same compatibility guarantees as the standard library. Use with caution.".into(),
            ..lsp::Diagnostic::default()
        })], diagnostics);
    }

    #[test]
    fn contrib_lint_check_with_alias() {
        let fluxscript = r#"import influxdb2 "contrib/jsternberg/influxdb"

influxdb2.select(
    from: "example-bucket",
    start: -1d,
    stop: now(),
    m: "example-measurement",
    fields: [],
    where: (r) => true,
    host: "https://example.com",
    org: "example-org",
    token: "MySuP3rSecr3Tt0k3n",
)
"#;
        let package = get_package(&fluxscript);

        let diagnostics = contrib_lint(&package);

        assert_eq!(vec![(Some("script.flux".to_string()), lsp::Diagnostic {
            range: lsp::Range {
                start: lsp::Position {
                    line: 2, character: 0,
                },
                end : lsp::Position {
                    line: 12, character: 1,
                },
            },
            severity: Some(lsp::DiagnosticSeverity::HINT),
            message: "contrib packages are user-contributed, and do not carry with them the same compatibility guarantees as the standard library. Use with caution.".into(),
            ..lsp::Diagnostic::default()
        })], diagnostics);
    }

    #[test]
    fn no_influxdb_identifiers_no_v() {
        let fluxscript = r#"import "strings"

v = {timeRangeStart: -12h, timeRangeStop: 12h}

from(bucket: "my-bucket")
  |> range(start: v.timeRangeStart, stop: v.timeRangeStop)
"#;
        let package = get_package(&fluxscript);

        let diagnostics = no_influxdb_identifiers(&package);

        assert_eq!(vec![(Some("script.flux".to_string()), lsp::Diagnostic {
            range: lsp::Range {
                start: lsp::Position {
                    line: 2, character: 0,
                },
                end : lsp::Position {
                    line: 2, character: 1,
                },
            },
            severity: Some(lsp::DiagnosticSeverity::WARNING),
            message: "Avoid using `v` as an identifier name. In some InfluxDB contexts, it may be provided at runtime.".into(),
            ..lsp::Diagnostic::default()
        })], diagnostics);
    }

    #[test]
    fn no_influxdb_identifiers_no_task() {
        let fluxscript = r#"import "strings"

task = "abc123"
"#;
        let package = get_package(&fluxscript);

        let diagnostics = no_influxdb_identifiers(&package);

        assert_eq!(vec![(Some("script.flux".to_string()), lsp::Diagnostic {
            range: lsp::Range {
                start: lsp::Position {
                    line: 2, character: 0,
                },
                end : lsp::Position {
                    line: 2, character: 4,
                },
            },
            severity: Some(lsp::DiagnosticSeverity::WARNING),
            message: "Avoid using `task` as an identifier name. In some InfluxDB contexts, it may be provided at runtime.".into(),
            ..lsp::Diagnostic::default()
        })], diagnostics);
    }

    #[test]
    fn no_influxdb_identifiers_no_params() {
        let fluxscript = r#"import "strings"

params = "abc123"
"#;
        let package = get_package(&fluxscript);

        let diagnostics = no_influxdb_identifiers(&package);

        assert_eq!(vec![(Some("script.flux".to_string()), lsp::Diagnostic {
            range: lsp::Range {
                start: lsp::Position {
                    line: 2, character: 0,
                },
                end : lsp::Position {
                    line: 2, character: 6,
                },
            },
            severity: Some(lsp::DiagnosticSeverity::WARNING),
            message: "Avoid using `params` as an identifier name. In some InfluxDB contexts, it may be provided at runtime.".into(),
            ..lsp::Diagnostic::default()
        })], diagnostics);
    }
}
