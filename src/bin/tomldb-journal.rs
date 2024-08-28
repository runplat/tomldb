use clap::Parser;
use tomldb::TableAction;

fn main() {
    let cli = TomldbJournal::parse_from(tomldb::split_args("tomldb-journal insert key -- 'value'").unwrap());

    println!("{:#?}", cli.action);
}

/// tomldb journal command line tool
/// 
/// Provides plumbing for journal actions
#[derive(Parser)]
struct TomldbJournal {
    /// Table action that is being journaled
    #[clap(subcommand)]
    action: TableAction,
}