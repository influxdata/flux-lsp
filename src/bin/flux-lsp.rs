use std::fs::OpenOptions;

use clap::{App, Arg};
use log::{debug, warn};
use lspower::{LspService, Server};
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};

use flux_lsp::LspServer;

#[async_std::main]
async fn main() {
    let matches = App::new("flux-lsp")
        .version("2.0")
        .author("Flux Developers <flux-developers@influxdata.com>")
        .about("LSP server for the Flux programming language")
        .arg(
            Arg::with_name("disable_folding")
                .long("disable-folding")
                .help("Turn folding off (used for editors with built-in folding")
                .takes_value(false))
        .arg(
            Arg::with_name("log_file")
            .short("l")
            .long("log-file")
            .help("Path to write a debug log file")
            .takes_value(true))
        .arg(
            Arg::with_name("url")
            .short("u")
            .long("url")
            .help("Base url for influxdb instance")
            .takes_value(true))
        .arg(
            Arg::with_name("token")
            .short("t")
            .long("token")
            .help("Token for influxdb instance")
            .takes_value(true))
        .arg(
            Arg::with_name("org")
            .short("o")
            .long("org")
            .help("Organization for influxdb instance")
            .takes_value(true))
        .get_matches();

    if matches.is_present("log_file") {
        let log_path = matches.value_of("log_file").unwrap();
        CombinedLogger::init(vec![WriteLogger::new(
            LevelFilter::Debug,
            Config::default(),
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .unwrap(),
        )])
        .unwrap();
    }

    let disable_folding = matches.is_present("disable_folding");
    let influxdb_url = match matches.value_of("url") {
        Some(value) => {
            warn!("url parameter specified but currently unused");
            Some(String::from(value))
        }
        None => None,
    };
    let token = match matches.value_of("token") {
        Some(value) => {
            warn!("token parameter specified but currently unused");
            Some(String::from(value))
        }
        None => None,
    };
    let org = match matches.value_of("org") {
        Some(value) => {
            warn!("org parameter specified but currently unused");
            Some(String::from(value))
        }
        None => None,
    };

    debug!("Starting lsp client");
    let stdin = async_std::io::stdin();
    let stdout = async_std::io::stdout();

    let (service, messages) = LspService::new(|_client| {
        let mut server = LspServer::default();
        if disable_folding {
            server = server.disable_folding();
        }
        if let Some(value) = influxdb_url {
            server = server.with_influxdb_url(value);
        }
        if let Some(value) = token {
            server = server.with_token(value);
        }
        if let Some(value) = org {
            server = server.with_org(value);
        }
        server
    });
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
}
