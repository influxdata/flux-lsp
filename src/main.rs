use flux_lsp_lib::loggers::FileLogger;
use flux_lsp_lib::Server;

use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let logger =
        Rc::new(RefCell::new(FileLogger::new("lsp.log").unwrap()));

    let mut server = Server::with_stdio(logger);
    server.start();
}
