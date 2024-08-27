use clap::Parser;
use tomldb::TableAction;

fn main() {
    let cli = Cli::parse();

    println!("{:#?}", cli.action);
}

// TODO: This should actually be the journal cli
#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    action: TableAction,
}