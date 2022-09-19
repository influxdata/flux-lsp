/// Tools for working with the Flux language and APIs for bridging
/// the gap between Flux language data structures and the needs of the LSP.
///
/// The purpose of this module is to be the single source of truth for all
/// things libflux. No other part of this library should
use std::cmp::Ordering;

use flux::semantic::types::MonoType;
use lspower::lsp;

use std::iter::Iterator;

lazy_static::lazy_static! {
    pub static ref STDLIB: Stdlib = Stdlib(flux::imports().expect("Could not initialize stdlib."));
    pub static ref UNIVERSE: Package = Package::new("builtin", flux::prelude().expect("Could not initialize prelude"));
}

/// Stdlib serves as the API for querying the flux stdlib.
///
/// The flux stdlib is a collection of packages, and this interface
/// provides a method for querying those packages.
pub struct Stdlib(flux::semantic::import::Packages);

impl Stdlib {
    /// Get all packages from the stdlib.
    pub fn packages(&self) -> impl Iterator<Item = Package> + '_ {
        self.0.iter().map(|(path, package)| {
            Package::new(path, package.clone())
        })
    }

    /// Get a package by path from the stdlib.
    pub fn package(&self, path: &str) -> Option<Package> {
        self.packages().find(|package| package.path == path)
    }

    /// Get all packages that fuzzy match on the needle.
    pub fn fuzzy_matches<'a>(
        &'a self,
        needle: &'a str,
    ) -> impl Iterator<Item = Package> + '_ {
        self.packages().filter(|package| {
            package
                .name
                .to_lowercase()
                .contains(needle.to_lowercase().as_str())
        })
    }
}

/// Package represents a flux package.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub path: String,
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
    pub fn functions(&self) -> impl Iterator<Item = Function> {
        if let MonoType::Record(record) = self.exports.typ().expr {
            record
                .fields()
                .filter(|property| {
                    matches!(&property.v, MonoType::Fun(_))
                        && !property.k.to_string().starts_with('_')
                })
                .map(|property| match &property.v {
                    MonoType::Fun(f) => Function {
                        name: property.k.to_string(),
                        expr: f.as_ref().clone(),
                    },
                    _ => unreachable!(
                        "Previous filter function failed"
                    ),
                })
                .collect::<Vec<Function>>()
                .into_iter()
        } else {
            unreachable!("Package is not actually a flux package.");
        }
    }

    /// Get a function by name from the package.
    pub fn function(&self, name: &str) -> Option<Function> {
        self.functions().find(|function| function.name == name)
    }

    /// Returns an iterator over all exports
    ///
    ///
    // `flux::semantic::types::PackageExports` also uses this
    // not-quite-an-iterator implementation. This serves as a layer in
    // front of that implementation.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&str, &flux::semantic::types::PolyType)> + '_
    {
        self.exports.iter().filter(|(key, _val)| {
            // Don't allow users to "discover" private-ish functionality, i.e. symbols
            // that start with an underscore.
            !key.starts_with('_')
        })
    }

    /// Get the package monotype.
    pub fn typ(&self) -> flux::semantic::types::MonoType {
        self.exports.typ().expr
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
pub struct Function {
    pub name: String,
    expr: flux::semantic::types::Function,
}

impl std::cmp::Ord for Function {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl std::cmp::PartialOrd for Function {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Function {}

impl Function {
    /// Get signature information for a flux function.
    pub fn signature_information(
        &self,
    ) -> Vec<lsp::SignatureInformation> {
        let required: Vec<String> =
            self.expr.req.keys().map(String::from).collect();
        let optional: Vec<String> =
            self.expr.opt.keys().map(String::from).collect();
        let mut result = vec![required.clone()];

        let mut combos = vec![];
        let length = optional.len();
        for i in 1..length {
            let c: Vec<Vec<String>> =
                combinations::Combinations::new(optional.clone(), i)
                    .collect();
            combos.extend(c);
        }
        combos.push(optional);

        for l in combos {
            let mut arguments = required.clone();
            arguments.extend(l.clone());

            result.push(arguments);
        }

        result
            .into_iter()
            .map(|arguments| lsp::SignatureInformation {
                label: {
                    let args = arguments
                        .iter()
                        .map(|x| format!("{}: ${}", x, x))
                        .collect::<Vec<String>>()
                        .join(" , ");

                    let result = format!("{}({})", self.name, args);

                    result
                },
                parameters: Some({
                    arguments
                        .iter()
                        .map(|x| lsp::ParameterInformation {
                            label: lsp::ParameterLabel::Simple(
                                format!("${}", x),
                            ),
                            documentation: None,
                        })
                        .collect()
                }),
                documentation: None,
                active_parameter: None,
            })
            .collect()
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
              "experimental/date/boundaries",
              "experimental/geo",
              "experimental/http",
              "experimental/http/requests",
              "experimental/influxdb",
              "experimental/iox",
              "experimental/json",
              "experimental/mqtt",
              "experimental/oee",
              "experimental/polyline",
              "experimental/prometheus",
              "experimental/query",
              "experimental/record",
              "experimental/table",
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
              "internal/testing",
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
                &STDLIB
                    .packages()
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
        let mut functions = UNIVERSE
            .functions()
            .map(|function| function.name.clone())
            .collect::<Vec<String>>();
        functions.sort();
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
            &serde_json::to_string_pretty(&functions).unwrap(),
        );
    }

    /// All functions from a package can be fetched
    #[test]
    fn csv_package_functions() {
        let csv = STDLIB.package("csv").unwrap();

        let functions = csv.functions();

        expect_test::expect![[r#"
            [
              "from"
            ]"#]]
        .assert_eq(
            &serde_json::to_string_pretty(
                &functions
                    .map(|function| function.name.clone())
                    .collect::<Vec<String>>(),
            )
            .unwrap(),
        );
    }

    #[test]
    fn function_parameters() {
        let from =
            STDLIB.package("csv").unwrap().function("from").unwrap();

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
            STDLIB.package("csv").unwrap().function("from").unwrap();

        expect_test::expect![[r#"
            [
              {
                "label": "from()",
                "parameters": []
              },
              {
                "label": "from(csv: $csv)",
                "parameters": [
                  {
                    "label": "$csv"
                  }
                ]
              },
              {
                "label": "from(file: $file)",
                "parameters": [
                  {
                    "label": "$file"
                  }
                ]
              },
              {
                "label": "from(mode: $mode)",
                "parameters": [
                  {
                    "label": "$mode"
                  }
                ]
              },
              {
                "label": "from(csv: $csv , file: $file)",
                "parameters": [
                  {
                    "label": "$csv"
                  },
                  {
                    "label": "$file"
                  }
                ]
              },
              {
                "label": "from(csv: $csv , mode: $mode)",
                "parameters": [
                  {
                    "label": "$csv"
                  },
                  {
                    "label": "$mode"
                  }
                ]
              },
              {
                "label": "from(file: $file , mode: $mode)",
                "parameters": [
                  {
                    "label": "$file"
                  },
                  {
                    "label": "$mode"
                  }
                ]
              },
              {
                "label": "from(csv: $csv , file: $file , mode: $mode)",
                "parameters": [
                  {
                    "label": "$csv"
                  },
                  {
                    "label": "$file"
                  },
                  {
                    "label": "$mode"
                  }
                ]
              }
            ]"#]].assert_eq(
            &serde_json::to_string_pretty(
                &from.signature_information(),
            )
            .unwrap(),
        );
    }
}
