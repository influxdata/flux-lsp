use flux_lsp_lib::loggers;
use flux_lsp_lib::Server;

use clap::{App, Arg, ArgMatches};
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let flags: ArgMatches = App::new("flux-lsp")
        .version("0.0.1")
        .arg(
            Arg::with_name("logfile")
                .help(
                    "sets the path of logfile, default won't log anything",
                )
                .short("l")
                .long("logfile")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("disable-folding")
            .help("set this flag to disable the range folding capacity")
            .long("disable-folding")
            .takes_value(false),
        )
        .get_matches();
    let logger: Rc<RefCell<dyn loggers::Logger>>;
    let disable_folding = flags.is_present("disable-folding");
    if let Some(ref logfile) = flags.value_of("logfile") {
        logger = Rc::new(RefCell::new(
            loggers::FileLogger::new(logfile).unwrap(),
        ));
    } else {
        logger =
            Rc::new(RefCell::new(loggers::DefaultLogger::default()));
    }
    let mut server = Server::with_stdio(logger, disable_folding);
    server.start();
}
