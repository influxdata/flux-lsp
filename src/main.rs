use flux_lsp_lib::Server;

use clap::{App, Arg, ArgMatches};

fn main() {
    let flags: ArgMatches = App::new("flux-lsp")
        .version("0.0.4")
        .arg(
            Arg::with_name("disable-folding")
            .help("set this flag to disable the range folding capacity")
            .long("disable-folding")
            .takes_value(false),
        )
        .get_matches();
    let disable_folding = flags.is_present("disable-folding");
    let mut server = Server::with_stdio(disable_folding);
    server.start();
}
