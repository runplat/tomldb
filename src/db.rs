use anyhow::anyhow;
use fs4::tokio::AsyncFileExt;
use futures::TryFutureExt;
use std::{
    ops::{Index, IndexMut},
    path::PathBuf,
};
use tokio::{
    fs::{File, OpenOptions},
    io::*,
    select,
    task::spawn_blocking,
};
use tokio_util::sync::CancellationToken;
use toml_edit::{DocumentMut, Item};

use crate::{args::TableAction, Result, TableArgs};

/// Struct containing database settings and file paths
pub struct Database {
    /// Path to a data file
    data_path: PathBuf,
    /// Path to a journal file
    journal_path: PathBuf,
}

impl Database {
    /// Starts a transaction
    pub fn start_transaction(&self) -> Transaction {
        Transaction::default()
    }
}

/// Variants of database transactions
pub enum Transaction {
    Empty {
        /// Cancellation token representing this transaction
        cancellation: CancellationToken,
    },
    Read {
        /// `read` pointer to data file w/ a shared-lock
        data: File,
    },
    Write {
        /// `read`, `write`, `append` file pointer to journal file w/ an exclusive-lock
        journal: Option<File>,
    },
    Commit {
        /// `read`, `write`, `append` file pointer to journal file w/ an exclusive-lock
        journal: File,
        /// `write`, `truncate` file pointer to data file w/ an exclusive-lock
        data: File,
    },
}

/// Wraps a toml_edit::DocumentMut so that changes can be recorded
#[derive(Default)]
pub struct Journal {
    /// Document being modified
    doc: DocumentMut,
    /// Current args
    pending: Vec<TableArgs>,
    /// Evaluated
    evaluated: Vec<(TableAction, TableArgs)>,
}

impl Journal {
    /// Returns an iterator over the current mutations
    pub fn iter_pending(&self) -> impl Iterator<Item = &TableArgs> {
        self.pending.iter()
    }

    pub fn table(&mut self, table: &str) -> &mut TableArgs {
        self.push_change({
            let mut args = TableArgs::default();
            args.set_table(table);
            args
        });
        self.pending.last_mut().expect("should exist just added")
    }

    /// Pushes a change
    pub fn push_change(&mut self, args: TableArgs) {
        self.pending.push(args);
    }

    /// Evaluates the current table args
    pub fn evaluate_args(&mut self) {
        for args in self.pending.drain(..) {
            match args.eval(&mut self.doc) {
                Ok(action) => {
                    self.evaluated.push((action, args));
                }
                Err(err) => {
                    eprintln!("Error {err:?}");
                }
            }
        }
    }

    /// Consumes the journal and commits the current journal state
    ///
    /// If the transaction is in the Commit state, than this commit **MUST** complete successfully,
    /// However, if the transaction is in the Write state, than this commit will return an error if a lock cannot immediately be
    /// acquired from the data file.
    pub async fn commit(self, mut tx: Transaction) -> crate::Result<()> {
        match &mut tx {
            // In this state, the commit must be guranteed since a lock has already been acquired on all data
            Transaction::Commit { journal, data } => {
                for (a, _) in self.evaluated {
                    journal.write(format!("{a}\n").as_bytes()).await?;
                }
                data.write_all(self.doc.to_string().as_bytes()).await?;
                Ok(())
            }
            // In this state, the commit is not guranteed since a lock has not yet been taken on the data file
            Transaction::Write { journal } => Ok(()),
            _ => Err(anyhow!(
                "Expecting tx to be either in the Commit or Write state"
            )),
        }
    }
}

impl Transaction {
    /// Consumes an empty transaction to create a read transaction
    pub async fn read(self, db: &Database) -> Result<Self> {
        match &self {
            Transaction::Empty { cancellation } => {
                let opening =
                    Self::lock_file(&db.data_path, OpenOptions::new().read(true).clone(), true);

                select! {
                    _ = cancellation.cancelled() => {
                        Err(anyhow!("Transaction cancelled"))
                    },
                    data = opening => {
                        Ok(Transaction::Read { data: data? })
                    }
                }
            }
            _ => Err(anyhow!(
                "Cannot pivot to a read transaction from a non-empty transaction"
            )),
        }
    }

    /// Consumes an empty transaction to create a write transaction
    ///
    /// A write transaction succeeds when an read/exclusive-lock on the journal can be claimed
    pub async fn write(self, db: &Database) -> Result<(Self, Journal)> {
        match &self {
            Transaction::Empty { cancellation } => {
                let opening = Self::lock_file(
                    &db.journal_path,
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .append(true)
                        .to_owned(),
                    false,
                );

                select! {
                    _ = cancellation.cancelled() => {
                        Err(anyhow!("Transaction cancelled"))
                    },
                    journal = opening => {
                        let journal = journal?;
                        Ok((Transaction::Write { journal: Some(journal) }, Journal::default()))
                    }
                }
            }
            _ => Err(anyhow!(
                "Cannot pivot to a write transaction from a non-empty transaction"
            )),
        }
    }

    /// Consumes a write transaction and tries to convert to a commit transaction
    pub async fn commit(mut self, db: &Database) -> Result<Self> {
        match &mut self {
            Transaction::Write { journal } => {
                // 1) Update the journal and unlock
                // 2) Get a write exclusive lock on
                // Ok(Self::Commit { journal, data: () })

                if let Some(journal) = journal.take() {
                    Ok(Self::Commit {
                        journal,
                        data: Self::lock_file(
                            &db.data_path,
                            OpenOptions::new().write(true).truncate(true).to_owned(),
                            false,
                        )
                        .await?,
                    })
                } else {
                    Err(anyhow!("Cannot commit without a journal file pointer"))
                }
            }
            _ => Err(anyhow!("Can only commit from a write transaction")),
        }
    }

    async fn lock_file(
        path: &PathBuf,
        options: tokio::fs::OpenOptions,
        shared: bool,
    ) -> Result<File> {
        let file = options
            .open(path)
            .and_then(|o| {
                spawn_blocking(move || {
                    if shared {
                        o.lock_shared()?;
                    } else {
                        o.lock_exclusive()?;
                    }
                    Ok::<_, std::io::Error>(o)
                })
                .map_err(|e| std::io::Error::other(e))
            })
            .await?;

        Ok(file?)
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::Empty {
            cancellation: CancellationToken::new(),
        }
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        match self {
            Transaction::Empty { cancellation } => {
                cancellation.cancel();
            }
            Transaction::Read { data } => {
                let _ = data.unlock();
            }
            Transaction::Write {
                journal: Some(journal),
            } => {
                let _ = journal.unlock();
            }
            Transaction::Commit { journal, data } => {
                let _ = journal.unlock();
                let _ = data.unlock();
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_empty_db() {
    tokio::fs::create_dir_all(".test/test_empty_db")
        .await
        .unwrap();

    let data_path = ".test/test_empty_db/config.toml";
    let journal = ".test/test_empty_db/config.toml.journal";

    std::fs::write(data_path, "").unwrap();
    std::fs::write(journal, "").unwrap();

    let db = Database {
        data_path: data_path.into(),
        journal_path: journal.into(),
    };

    let tx = db.start_transaction();
    let (tx, mut journal) = tx.write(&db).await.unwrap();
    let tx = tx.commit(&db).await.unwrap();
    {
        journal.table("test").set_kvp("value-2", 3.14f32);
        journal.table("test").set_kvp("value", "hello world");
        let to_remove = journal.table("test");
        to_remove.set_kvp("value", "hello world");
        to_remove.set_remove(true);
    }
    journal.evaluate_args();
    journal.commit(tx).await.unwrap();
    ()
}
