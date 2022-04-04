#![allow(clippy::unwrap_used)]
use std::fs::OpenOptions;

use clap::Parser;
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};
use tower_lsp::{LspService, Server};

use flux_lsp::LspServer;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, short, help = "Path to write a debug log file")]
    log_file: Option<String>,
}

#[async_std::main]
async fn main() {
    let matches = Args::parse();

    if let Some(log_path) = matches.log_file {
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

    log::debug!("Starting lsp client");
    let stdin = async_std::io::stdin();
    let stdout = async_std::io::stdout();

    let (service, messages) =
        LspService::new(|client| LspServer::new(Some(client)));
    Server::new(stdin, stdout, messages).serve(service).await;
}
