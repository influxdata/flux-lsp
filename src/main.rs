use flux_lsp_lib::loggers::FileLogger;
use flux_lsp_lib::Server;

fn main() {
    let server_log = FileLogger::new("lsp.log").unwrap();
    let handler_log = FileLogger::new("lsp.log").unwrap();

    let mut server = Server::with_stdio();
    server.logger = Box::new(server_log);
    server.handler.logger = Box::new(handler_log);

    server.start();
}
