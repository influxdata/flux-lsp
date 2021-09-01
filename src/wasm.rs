#![allow(dead_code, unused_imports)]
use std::ops::Add;
use std::str;

use flux::ast::File;
use flux::formatter::convert_to_string;
use flux::{docs, docs_json};
use js_sys::{Function, Promise};
use serde::{Deserialize, Serialize};
use tower_service::Service;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::LspServer;
use flux::parser::Parser;

fn wrap_message(s: String) -> String {
    let st = s.clone();
    let result = st.as_bytes();
    let size = result.len();

    format!("Content-Length: {}\r\n\r\n{}", size, s)
}

#[derive(Serialize)]
struct ResponseError {
    code: u32,
    message: String,
}

#[wasm_bindgen]
#[derive(Deserialize)]
struct ServerResponse {
    #[allow(dead_code)]
    message: Option<String>,
    #[allow(dead_code)]
    error: Option<String>,
}

#[wasm_bindgen]
impl ServerResponse {
    #[allow(dead_code)]
    pub fn get_message(&self) -> Option<String> {
        self.message.clone()
    }

    #[allow(dead_code)]
    pub fn get_error(&self) -> Option<String> {
        self.error.clone()
    }
}

#[derive(Serialize)]
struct ServerError {
    id: u32,
    error: ResponseError,
    jsonrpc: String,
}

#[wasm_bindgen]
pub struct Server {
    service: lspower::LspService,
}

#[wasm_bindgen]
impl Server {
    #[wasm_bindgen(constructor)]
    pub fn new(
        disable_folding: bool,
        _support_multiple_files: bool,
    ) -> Self {
        let (service, _messages) =
            lspower::LspService::new(|_client| {
                let mut server = LspServer::default();
                if disable_folding {
                    server = server.disable_folding();
                }
                server
            });

        Server { service }
    }

    pub fn process(&mut self, msg: String) -> Promise {
        let json_contents: String =
            msg.lines().skip(2).fold(String::new(), |c, l| c.add(l));

        let message: lspower::jsonrpc::Incoming =
            serde_json::from_str(&json_contents).unwrap();
        let call = self.service.call(message);
        future_to_promise(async move {
            match call.await {
                Ok(result) => match result {
                    Some(result_inner) => match result_inner {
                        lspower::jsonrpc::Outgoing::Response(
                            response,
                        ) => match serde_json::to_string(&response) {
                            Ok(value) => {
                                Ok(JsValue::from(ServerResponse {
                                    message: Some(wrap_message(
                                        value,
                                    )),
                                    error: None,
                                }))
                            }
                            Err(err) => {
                                Ok(JsValue::from(ServerResponse {
                                    message: None,
                                    error: Some(format!("{}", err)),
                                }))
                            }
                        },
                        lspower::jsonrpc::Outgoing::Request(
                            _client_request,
                        ) => {
                            panic!("Outgoing requests from server to client are not implemented");
                        }
                    },
                    None => {
                        // Some endpoints don't have results,
                        // e.g. textDocument/didOpen
                        Ok(JsValue::from(ServerResponse {
                            message: None,
                            error: None,
                        }))
                    }
                },
                Err(err) => Ok(JsValue::from(ServerResponse {
                    message: None,
                    error: Some(format!("{}", err)),
                })),
            }
        })
    }

    pub fn register_buckets_callback(&mut self, _f: Function) {}
    pub fn register_measurements_callback(&mut self, _f: Function) {}
    pub fn register_tag_keys_callback(&mut self, _f: Function) {}
    pub fn register_tag_values_callback(&mut self, _f: Function) {}
}

/// Parses a string representation of a json file and returns the corresponding JsValue representation
#[wasm_bindgen]
pub fn parse(s: &str) -> JsValue {
    let mut p = Parser::new(s);
    let file = p.parse_file(String::from(""));

    JsValue::from_serde(&file).unwrap()
}

/// Format a JS file.
#[wasm_bindgen]
pub fn format_from_js_file(js_file: JsValue) -> String {
    if let Ok(file) = js_file.into_serde::<File>() {
        if let Ok(converted) = convert_to_string(&file) {
            return converted;
        }
    }
    "".to_string()
}

/// Gets json docs for the entire stdlib
#[wasm_bindgen]
pub fn get_all_docs() -> JsValue {
    let d = docs_json().unwrap();
    JsValue::from_serde(std::str::from_utf8(&d).unwrap()).unwrap()
}

/// Gets json docs from a Flux identifier
#[wasm_bindgen]
pub fn get_json_documentation(flux_path: &str) -> JsValue {
    let d = docs();
    let mut doc = JsValue::UNDEFINED;

    for i in &d {
        // look for the given identifier
        if flux_path == i.path {
            // return that doc package
            let param = serde_json::to_string(i).unwrap();
            doc = JsValue::from_serde(&param).unwrap();
            break;
        }
    }

    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux::semantic::bootstrap::Doc;
    use flux::semantic::bootstrap::*;
    use wasm_bindgen_test::*;

    ///Retrieve full PackageDoc documentation for a single flux identifier
    #[wasm_bindgen_test]
    pub fn json_docs_test() {
        let csv_doc = r#"{"path":"csv","name":"csv","headline":"Package csv provides tools for working with data in annotated CSV format.","description":null,"members":{"from":{"kind":"Function","name":"from","headline":"from is a function that retrieves data from a comma separated value (CSV) data source. ","description":"A stream of tables are returned, each unique series contained within its own table. Each record in the table represents a single point in the series. ## Query anotated CSV data from file\n```\nimport \"csv\"\n\ncsv.from(file: \"path/to/data-file.csv\")\n```\n\n## Query raw data from CSV file\n```\nimport \"csv\"\n\ncsv.from(\n  file: \"/path/to/data-file.csv\",\n  mode: \"raw\"\n)\n```\n\n## Query an annotated CSV string\n```\nimport \"csv\"\n\ncsvData = \"\n#datatype,string,long,dateTime:RFC3339,dateTime:RFC3339,dateTime:RFC3339,string,string,double\n#group,false,false,false,false,false,false,false,false\n#default,,,,,,,,\n,result,table,_start,_stop,_time,region,host,_value\n,mean,0,2018-05-08T20:50:00Z,2018-05-08T20:51:00Z,2018-05-08T20:50:00Z,east,A,15.43\n,mean,0,2018-05-08T20:50:00Z,2018-05-08T20:51:00Z,2018-05-08T20:50:20Z,east,B,59.25\n,mean,0,2018-05-08T20:50:00Z,2018-05-08T20:51:00Z,2018-05-08T20:50:40Z,east,C,52.62\n\"\n\ncsv.from(csv: csvData)\n\n```\n\n## Query a raw CSV string\n```\nimport \"csv\"\n\ncsvData = \"\n_start,_stop,_time,region,host,_value\n2018-05-08T20:50:00Z,2018-05-08T20:51:00Z,2018-05-08T20:50:00Z,east,A,15.43\n2018-05-08T20:50:00Z,2018-05-08T20:51:00Z,2018-05-08T20:50:20Z,east,B,59.25\n2018-05-08T20:50:00Z,2018-05-08T20:51:00Z,2018-05-08T20:50:40Z,east,C,52.62\n\"\n\ncsv.from(\n  csv: csvData,\n  mode: \"raw\"\n)\n```\n\n","parameters":[{"name":"csv","headline":" is CSV data.","description":"Supports anonotated CSV or raw CSV. Use mode to specify the parsing mode.","required":false},{"name":"file","headline":" is the file path of the CSV file to query.","description":"The path can be absolute or relative. If relative, it is relative to the working directory of the  fluxd  process. The CSV file must exist in the same file system running the  fluxd  process.","required":false},{"name":"mode","headline":" is the CSV parsing mode. Default is annotations.","description":"Available annotation modes: annotations: Use CSV notations to determine column data types. raw: Parse all columns as strings and use the first row as the header row and all subsequent rows as data.","required":false}],"flux_type":"(?csv:string, ?file:string, ?mode:string) => [A] where A: Record","link":"https://docs.influxdata.com/influxdb/cloud/reference/flux/stdlib/csv/from"}},"link":"https://docs.influxdata.com/influxdb/cloud/reference/flux/stdlib/csv"}"#;
        let want = JsValue::from_serde(csv_doc).unwrap();
        let got = get_json_documentation("csv");
        assert_eq!(want, got);
    }

    ///Retrieve full vector of PackageDoc documentation for entire stdlib
    #[wasm_bindgen_test]
    pub fn all_docs_test() {
        let docs: Vec<PackageDoc> =
            serde_json::from_slice(&docs_json().unwrap()).unwrap();
        let first = &docs[0].path;
        assert_eq!(first.to_string(), "array");
        let end = &docs[docs.len() - 1].path;
        assert_eq!(end.to_string(), "universe");
    }
}
