mod args;
mod types;
mod db;

pub use args::TableArgs;
pub use args::TableAction;

pub use types::Types;

pub type Result<T> = anyhow::Result<T>;