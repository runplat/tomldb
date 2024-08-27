use clap::Parser;
use tomldb::TableAction;

fn main() {
    let cli = Cli::parse_from(tomldb::split_cmd("tomldb-journal insert key -- 'value'").unwrap());

    println!("{:#?}", cli.action);
}

// TODO: This should actually be the journal cli
#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    action: TableAction,
}