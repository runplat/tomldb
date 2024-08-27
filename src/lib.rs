mod args;
mod db;
mod types;

pub use args::TableAction;
pub use args::TableArgs;

pub use types::Types;

pub type Result<T> = anyhow::Result<T>;

/// Splits a cmd into arguments mostly using shlex
/// 
/// If the sequence " -- " is found, splits on that sequence into a head and tail segment.
/// The tail segment will be trimmed and returned, however it's inputs will be preserved. The result
/// of shlex will be popped once to remove shlex's last argument and replaced by the tail
pub fn split_cmd(cmd: &str) -> Option<Vec<String>> {
    if cmd.contains(" -- ") {
        shlex::split(cmd).zip(cmd.split_once(" -- ")).map(|(mut shlex, (_, tail))| {
            shlex.pop();
            shlex.push(tail.trim().to_string());
            shlex
        })
    } else {
        shlex::split(cmd)
    }
}