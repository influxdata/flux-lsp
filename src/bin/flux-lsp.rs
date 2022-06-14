#![allow(clippy::unwrap_used)]
use std::fs::OpenOptions;

use clap::Parser;
use lspower::{LspService, Server};
use simplelog::{
    CombinedLogger, Config, LevelFilter, SimpleLogger, WriteLogger,
};
use tokio::net::{TcpListener, UnixListener};

use flux_lsp::LspServer;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, short, help = "Path to write a debug log file")]
    log_file: Option<String>,
    #[clap(
        long,
        help = "I/O communication channel to use, stdin, tcp, unix (defaults to \"stdin\")"
    )]
    channel: Option<String>,
    #[clap(
        long,
        help = "TCP address to bind when channel is \"tcp\" (defaults to :5001)"
    )]
    addr: Option<String>,
    #[clap(
        long,
        help = "Path to unix socket when channel is \"unix\" (defaults to /tmp/flux-lsp-sock.unix)"
    )]
    path: Option<String>,
}

#[tokio::main]
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

    let (service, messages) =
        LspService::new(|client| LspServer::new(Some(client)));

    let channel =
        matches.channel.unwrap_or_else(|| "stdio".to_string());
    match channel.as_str() {
        "stdio" => {
            log::debug!("Communicating using stdin/stdout");
            Server::new(tokio::io::stdin(), tokio::io::stdout())
                .interleave(messages)
                .serve(service)
                .await;
        }
        "tcp" => {
            SimpleLogger::init(LevelFilter::Debug, Config::default())
                .unwrap();
            let addr = matches
                .addr
                .unwrap_or_else(|| "127.0.0.1:5001".to_string());
            log::debug!("Communicating on tcp socket {}", addr);
            let listener = match TcpListener::bind(&addr).await {
                Ok(listener) => listener,
                Err(err) => {
                    log::error!("Cannot bind to {} {}", addr, err);
                    std::process::exit(1);
                }
            };
            let (stream, _) = listener.accept().await.unwrap();
            let (read, write) = tokio::io::split(stream);
            Server::new(read, write)
                .interleave(messages)
                .serve(service)
                .await;
        }
        "unix" => {
            SimpleLogger::init(LevelFilter::Debug, Config::default())
                .unwrap();
            let path = matches.path.unwrap_or_else(|| {
                "/tmp/flux-lsp-sock.unix".to_string()
            });
            log::debug!("Communicating on unix socket {}", path);
            let listener = match UnixListener::bind(&path) {
                Ok(listener) => listener,
                Err(err) => {
                    log::error!("Cannot bind to {} {}", path, err);
                    std::process::exit(1);
                }
            };
            let (stream, _) = listener.accept().await.unwrap();
            let (read, write) = tokio::io::split(stream);
            Server::new(read, write)
                .interleave(messages)
                .serve(service)
                .await;
        }
        _ => {
            eprintln!(
                "Unsupported communication channel: {}",
                channel
            );
            std::process::exit(1);
        }
    }
}
