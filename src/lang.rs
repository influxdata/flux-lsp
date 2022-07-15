/// Tools for working with the Flux language and APIs for bridging
/// the gap between Flux language data structures and the needs of the LSP.
///
/// The purpose of this module is to be the single source of truth for all
/// things libflux. No other part of this library should
use std::cmp::Ordering;

use flux::semantic::types::{MonoType, Record};
use lspower::lsp;

use std::collections::BTreeMap;
use std::iter::Iterator;

const BUILTIN_PACKAGE: &str = "builtin";
lazy_static::lazy_static! {
    pub static ref PRELUDE: flux::semantic::PackageExports = flux::prelude().expect("Could not initialize prelude.");
    pub static ref STDLIB: flux::semantic::import::Packages = flux::imports().expect("Could not initialize stdlib.");
    pub static ref STDLIB_: Stdlib = Stdlib(flux::imports().expect("Could not initialize stdlib."));
    pub static ref UNIVERSE: Package = Package::new("universe", flux::prelude().expect("Could not initialize prelude"));
}

/// Stdlib serves as the API for querying the flux stdlib.
///
/// The flux stdlib is a collection of packages, and this interface
/// provides a method for querying those packages.
pub struct Stdlib(flux::semantic::import::Packages);

impl Stdlib {
    /// Get all packages from the stdlib.
    pub fn packages(&self) -> Vec<Package> {
        self.0
            .iter()
            .map(|(path, package)| {
                Package::new(path, package.clone())
            })
            .collect()
    }

    /// Get a package by path from the stdlib.
    pub fn package(&self, path: &str) -> Option<Package> {
        self.packages()
            .iter()
            .filter(|package| package.path == path)
            .map(|package| package.clone())
            .next()
    }
}

/// Package represents a flux package.
#[derive(Debug, Clone)]
pub struct Package {
    name: String,
    path: String,
    exports: flux::semantic::PackageExports,
}

impl Package {
    fn new(
        path: &str,
        exports: flux::semantic::PackageExports,
    ) -> Self {
        Self {
            path: path.into(),
            name: path
                .split('/')
                .last()
                .expect("Received an unsupported package name")
                .into(),
            exports,
        }
    }

    /// Get all functions in the package.
    pub fn functions(&self) -> Vec<Function_> {
        if let MonoType::Record(record) = self.exports.typ().expr {
            let mut functions: Vec<Function_> = record
                .fields()
                .filter(|property| {
                    matches!(&property.v, MonoType::Fun(_))
                        && !property.k.to_string().starts_with('_')
                })
                .map(|property| match &property.v {
                    MonoType::Fun(f) => Function_ {
                        name: property.k.to_string(),
                        expr: f.as_ref().clone(),
                    },
                    _ => unreachable!(
                        "Previous filter function failed"
                    ),
                })
                .collect();
            // Sort the functions into alphabetical order, plz.
            functions.sort();
            functions
        } else {
            log::warn!("Package is not actually a flux package.");
            vec![]
        }
    }

    /// Get a function by name from the package.
    pub fn function(&self, name: &str) -> Option<Function_> {
        self.functions()
            .iter()
            .filter(|function| function.name == name)
            .map(|function| function.clone())
            .next()
    }
}

/// A flux function struct
///
/// This struct provides a bridge between the flux language function and
/// its lsp representations around completion, signature help, etc.
///
/// The contract here is that all public interfaces here return lsp data
/// structures. Any deviation from that contract should be considered technical
/// debt and handled accordingly.
#[derive(Clone, Debug)]
pub struct Function_ {
    pub name: String,
    expr: flux::semantic::types::Function,
}

impl std::cmp::Ord for Function_ {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl std::cmp::PartialOrd for Function_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Function_ {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Function_ {}

impl Function_ {
    fn signature_information(
        &self,
    ) -> Vec<lsp::SignatureInformation> {
        vec![]
    }

    pub fn parameters(&self) -> Vec<(String, MonoType)> {
        self.expr
            .req
            .iter()
            .chain(self.expr.opt.iter().map(|p| (p.0, &p.1.typ)))
            .chain(self.expr.pipe.as_ref().map(|p| (&p.k, &p.v)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    fn signature(&self) -> String {
        let required = self
            .expr
            .req
            .iter()
            // Sort args with BTree
            .collect::<BTreeMap<_, _>>()
            .iter()
            .map(|(&k, &v)| (k.clone(), format!("{}", v)))
            .collect::<Vec<_>>();

        let optional = self
            .expr
            .opt
            .iter()
            // Sort args with BTree
            .collect::<BTreeMap<_, _>>()
            .iter()
            .map(|(&k, &v)| (k.clone(), format!("{}", v.typ)))
            .collect::<Vec<_>>();

        let pipe = match &self.expr.pipe {
            Some(pipe) => {
                if pipe.k == "<-" {
                    vec![(pipe.k.clone(), format!("{}", pipe.v))]
                } else {
                    vec![(
                        format!("<-{}", pipe.k),
                        format!("{}", pipe.v),
                    )]
                }
            }
            None => vec![],
        };

        format!(
            "({}) -> {}",
            pipe.iter()
                .chain(required.iter().chain(optional.iter()))
                .map(|arg| format!("{}:{}", arg.0, arg.1))
                .collect::<Vec<_>>()
                .join(", "),
            self.expr.retn
        )
    }
}

pub fn get_package_name(name: &str) -> &str {
    name.split('/')
        .last()
        .expect("Invalid package path/name supplied")
}

pub fn create_function_signature(
    f: &flux::semantic::types::Function,
) -> String {
    let required = f
        .req
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| (k.clone(), format!("{}", v)))
        .collect::<Vec<_>>();

    let optional = f
        .opt
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| (k.clone(), format!("{}", v.typ)))
        .collect::<Vec<_>>();

    let pipe = match &f.pipe {
        Some(pipe) => {
            if pipe.k == "<-" {
                vec![(pipe.k.clone(), format!("{}", pipe.v))]
            } else {
                vec![(format!("<-{}", pipe.k), format!("{}", pipe.v))]
            }
        }
        None => vec![],
    };

    format!(
        "({}) -> {}",
        pipe.iter()
            .chain(required.iter().chain(optional.iter()))
            .map(|arg| format!("{}:{}", arg.0, arg.1))
            .collect::<Vec<_>>()
            .join(", "),
        f.retn
    )
}

fn record_fields(
    this: &Record,
) -> impl Iterator<Item = &flux::semantic::types::Property> {
    let mut record = Some(this);
    std::iter::from_fn(move || match record {
        Some(Record::Extension { head, tail }) => {
            match tail {
                MonoType::Record(tail) => record = Some(tail),
                _ => record = None,
            }
            Some(head)
        }
        _ => None,
    })
}

pub fn get_package_functions(name: &str) -> Vec<Function> {
    STDLIB
        .iter()
        .filter(|(_key, val)| {
            matches!(&val.typ().expr, MonoType::Record(_))
        })
        .flat_map(|(key, val)| match &val.typ().expr {
            MonoType::Record(record) => record_fields(record)
                .filter(|head| {
                    matches!(&head.v, MonoType::Fun(_))
                        && get_package_name(key) == name
                })
                .map(|head| match &head.v {
                    MonoType::Fun(f) => {
                        Function::new(head.k.to_string(), f)
                    }
                    _ => unreachable!("Previous filter failed"),
                })
                .collect::<Vec<Function>>(),
            _ => unreachable!("Previous filter failer"),
        })
        .collect()
}

pub fn get_stdlib_functions() -> Vec<FunctionInfo> {
    let builtins = PRELUDE
        .iter()
        .filter(|(_key, val)| matches!(&val.expr, MonoType::Fun(_)))
        .map(|(key, val)| match &val.expr {
            MonoType::Fun(f) => FunctionInfo::new(
                key.into(),
                f.as_ref(),
                BUILTIN_PACKAGE.into(),
            ),
            _ => unreachable!("Previous filter failed"),
        });

    let imported = STDLIB
        .iter()
        .filter(|(_key, val)| {
            matches!(&val.typ().expr, MonoType::Record(_))
        })
        .flat_map(|(key, val)| match &val.typ().expr {
            MonoType::Record(record) => record_fields(record)
                .filter(|property| {
                    matches!(&property.v, MonoType::Fun(_))
                })
                .map(|property| match &property.v {
                    MonoType::Fun(f) => FunctionInfo::new(
                        property.k.to_string(),
                        f.as_ref(),
                        get_package_name(key).into(),
                    ),
                    _ => unreachable!("Previous filter failed"),
                })
                .collect::<Vec<FunctionInfo>>(),
            _ => unreachable!("Previous filter failed"),
        });
    builtins.chain(imported.into_iter()).collect()
}

pub struct FunctionInfo {
    pub name: String,
    pub package_name: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

impl FunctionInfo {
    pub fn new(
        name: String,
        f: &flux::semantic::types::Function,
        package_name: String,
    ) -> Self {
        FunctionInfo {
            name,
            package_name,
            required_args: get_argument_names(&f.req),
            optional_args: get_optional_argument_names(&f.opt),
        }
    }

    pub fn signatures(&self) -> Vec<FunctionSignature> {
        let mut result = vec![FunctionSignature {
            name: self.name.clone(),
            arguments: self.required_args.clone(),
        }];

        let mut combos = vec![];
        let length = self.optional_args.len();
        for i in 1..length {
            let c: Vec<Vec<String>> =
                combinations::Combinations::new(
                    self.optional_args.clone(),
                    i,
                )
                .collect();
            combos.extend(c);
        }
        combos.push(self.optional_args.clone());

        for l in combos {
            let mut arguments = self.required_args.clone();
            arguments.extend(l.clone());

            result.push(FunctionSignature {
                name: self.name.clone(),
                arguments,
            });
        }

        result
    }
}

pub struct FunctionSignature {
    pub name: String,
    pub arguments: Vec<String>,
}

impl FunctionSignature {
    pub fn create_signature(&self) -> String {
        let args: String = self
            .arguments
            .iter()
            .map(|x| format!("{}: ${}", x, x))
            .collect::<Vec<String>>()
            .join(" , ");

        let result = format!("{}({})", self.name, args);

        result
    }

    pub fn create_parameters(
        &self,
    ) -> Vec<lsp::ParameterInformation> {
        self.arguments
            .iter()
            .map(|x| lsp::ParameterInformation {
                label: lsp::ParameterLabel::Simple(format!("${}", x)),
                documentation: None,
            })
            .collect()
    }
}

#[allow(clippy::implicit_hasher)]
pub fn get_argument_names(
    args: &std::collections::BTreeMap<String, MonoType>,
) -> Vec<String> {
    args.keys().map(String::from).collect()
}

#[allow(clippy::implicit_hasher)]
pub fn get_optional_argument_names(
    args: &std::collections::BTreeMap<
        String,
        flux::semantic::types::Argument<MonoType>,
    >,
) -> Vec<String> {
    args.keys().map(String::from).collect()
}

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Option<MonoType>)>,
}

impl Function {
    pub(crate) fn new(
        name: String,
        f: &flux::semantic::types::Function,
    ) -> Self {
        let params = f
            .req
            .iter()
            .chain(f.opt.iter().map(|p| (p.0, &p.1.typ)))
            .chain(f.pipe.as_ref().map(|p| (&p.k, &p.v)))
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();
        Self { name, params }
    }

    pub(crate) fn from_expr(
        name: String,
        expr: &flux::semantic::nodes::FunctionExpr,
    ) -> Self {
        let params = expr
            .params
            .iter()
            .map(|p| {
                (
                    p.key.name.to_string(),
                    expr.typ.parameter(&p.key.name).cloned(),
                )
            })
            .collect::<Vec<_>>();
        Self { name, params }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All stdlib packages are fetched.
    ///
    /// There is some logic that makes assumptions about flux packages,
    /// and this test ensures those assumptions don't cause a panic. This
    /// test is especially important as packages are added, as the new packages
    /// may press those assumptions.
    #[test]
    fn get_stdlib() {
        expect_test::expect![[r#"
            [
              "array",
              "bitwise",
              "contrib/RohanSreerama5/naiveBayesClassifier",
              "contrib/anaisdg/anomalydetection",
              "contrib/anaisdg/statsmodels",
              "contrib/bonitoo-io/alerta",
              "contrib/bonitoo-io/hex",
              "contrib/bonitoo-io/servicenow",
              "contrib/bonitoo-io/tickscript",
              "contrib/bonitoo-io/victorops",
              "contrib/bonitoo-io/zenoss",
              "contrib/chobbs/discord",
              "contrib/jsternberg/aggregate",
              "contrib/jsternberg/influxdb",
              "contrib/jsternberg/math",
              "contrib/rhajek/bigpanda",
              "contrib/sranka/opsgenie",
              "contrib/sranka/sensu",
              "contrib/sranka/teams",
              "contrib/sranka/telegram",
              "contrib/sranka/webexteams",
              "contrib/tomhollingworth/events",
              "csv",
              "date",
              "date/boundaries",
              "dict",
              "experimental",
              "experimental/aggregate",
              "experimental/array",
              "experimental/bigtable",
              "experimental/bitwise",
              "experimental/csv",
              "experimental/geo",
              "experimental/http",
              "experimental/http/requests",
              "experimental/influxdb",
              "experimental/iox",
              "experimental/json",
              "experimental/mqtt",
              "experimental/oee",
              "experimental/prometheus",
              "experimental/query",
              "experimental/record",
              "experimental/table",
              "experimental/universe",
              "experimental/usage",
              "generate",
              "http",
              "http/requests",
              "influxdata/influxdb",
              "influxdata/influxdb/monitor",
              "influxdata/influxdb/sample",
              "influxdata/influxdb/schema",
              "influxdata/influxdb/secrets",
              "influxdata/influxdb/tasks",
              "influxdata/influxdb/v1",
              "internal/boolean",
              "internal/debug",
              "internal/gen",
              "internal/influxql",
              "internal/location",
              "internal/promql",
              "internal/testutil",
              "interpolate",
              "join",
              "json",
              "kafka",
              "math",
              "pagerduty",
              "planner",
              "profiler",
              "pushbullet",
              "regexp",
              "runtime",
              "sampledata",
              "slack",
              "socket",
              "sql",
              "strings",
              "system",
              "testing",
              "testing/expect",
              "timezone",
              "types",
              "universe"
            ]"#]]
        .assert_eq(
            &serde_json::to_string_pretty(
                &STDLIB_
                    .packages()
                    .iter()
                    .map(|package| package.path.clone())
                    .collect::<Vec<String>>(),
            )
            .unwrap(),
        );
    }

    /// All universe functions are fetched.
    ///
    /// Universe is just a single Package, and thus can be navigated like any
    /// other package.
    #[test]
    fn get_universe_functions() {
        expect_test::expect![[r#"
            [
              "aggregateWindow",
              "bool",
              "bottom",
              "buckets",
              "bytes",
              "cardinality",
              "chandeMomentumOscillator",
              "columns",
              "contains",
              "count",
              "cov",
              "covariance",
              "cumulativeSum",
              "derivative",
              "die",
              "difference",
              "display",
              "distinct",
              "doubleEMA",
              "drop",
              "duplicate",
              "duration",
              "elapsed",
              "exponentialMovingAverage",
              "fill",
              "filter",
              "findColumn",
              "findRecord",
              "first",
              "float",
              "from",
              "getColumn",
              "getRecord",
              "group",
              "highestAverage",
              "highestCurrent",
              "highestMax",
              "histogram",
              "histogramQuantile",
              "holtWinters",
              "hourSelection",
              "increase",
              "int",
              "integral",
              "join",
              "kaufmansAMA",
              "kaufmansER",
              "keep",
              "keyValues",
              "keys",
              "last",
              "length",
              "limit",
              "linearBins",
              "logarithmicBins",
              "lowestAverage",
              "lowestCurrent",
              "lowestMin",
              "map",
              "max",
              "mean",
              "median",
              "min",
              "mode",
              "movingAverage",
              "now",
              "pearsonr",
              "pivot",
              "quantile",
              "range",
              "reduce",
              "relativeStrengthIndex",
              "rename",
              "sample",
              "set",
              "skew",
              "sort",
              "spread",
              "stateCount",
              "stateDuration",
              "stateTracking",
              "stddev",
              "string",
              "sum",
              "tableFind",
              "tail",
              "time",
              "timeShift",
              "timeWeightedAvg",
              "timedMovingAverage",
              "to",
              "toBool",
              "toFloat",
              "toInt",
              "toString",
              "toTime",
              "toUInt",
              "today",
              "top",
              "tripleEMA",
              "tripleExponentialDerivative",
              "truncateTimeColumn",
              "uint",
              "union",
              "unique",
              "wideTo",
              "window",
              "yield"
            ]"#]]
        .assert_eq(
            &serde_json::to_string_pretty(
                &UNIVERSE
                    .functions()
                    .iter()
                    .map(|function| function.name.clone())
                    .collect::<Vec<String>>(),
            )
            .unwrap(),
        );
    }

    /// All functions from a package can be fetched
    #[test]
    fn csv_package_functions() {
        let csv = STDLIB_.package("csv").unwrap();

        let functions = csv.functions();

        expect_test::expect![[r#"
            [
              "from"
            ]"#]]
        .assert_eq(
            &serde_json::to_string_pretty(
                &functions
                    .iter()
                    .map(|function| function.name.clone())
                    .collect::<Vec<String>>(),
            )
            .unwrap(),
        );
    }

    #[test]
    fn function_signature() {
        let from =
            STDLIB_.package("csv").unwrap().function("from").unwrap();

        assert_eq!(
            "(csv:string, file:string, mode:string) -> stream[A]",
            from.signature()
        );
    }

    #[test]
    fn function_parameters() {
        let from =
            STDLIB_.package("csv").unwrap().function("from").unwrap();

        expect_test::expect![[r#"
            [
              [
                "csv",
                "String"
              ],
              [
                "file",
                "String"
              ],
              [
                "mode",
                "String"
              ]
            ]"#]]
        .assert_eq(
            &serde_json::to_string_pretty(&from.parameters())
                .unwrap(),
        );
    }

    #[test]
    fn function_signature_information() {
        let from =
            STDLIB_.package("csv").unwrap().function("from").unwrap();

        expect_test::expect![[r#"[]"#]].assert_eq(
            &serde_json::to_string_pretty(
                &from.signature_information(),
            )
            .unwrap(),
        );
    }
}
