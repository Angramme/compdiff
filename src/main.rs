
use clap::Parser;
use compdiff::{cli::Cli, cli::handle_cli};

fn main() {
    let args = Cli::parse();

    handle_cli(args);
}
