#![allow(clippy::unwrap_used)]
use std::fs::OpenOptions;

use clap::{App, Arg};
use lspower::{LspService, Server};
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};

use flux_lsp::LspServerBuilder;

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

    log::debug!("Starting lsp client");
    let stdin = async_std::io::stdin();
    let stdout = async_std::io::stdout();

    let (service, messages) = LspService::new(|_client| {
        let mut builder = LspServerBuilder::default();
        if disable_folding {
            builder = builder.disable_folding();
        }
        builder.build()
    });
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
}
