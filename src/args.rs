use crate::Result;
use crate::Types;
use anyhow::anyhow;
use clap::{Args, Subcommand};
use std::fmt::Display;
use std::path::PathBuf;
use toml_edit::value;
use toml_edit::{DocumentMut, Table};

/// Struct containing arguments used to reference a document
#[derive(Default, Args, Debug, Clone)]
pub struct TableArgs {
    /// If set, will attempt to remove a key
    #[clap(long, short, action, default_value = "false")]
    remove: bool,
    /// If set, will execute actions that modify any existing configuration
    ///
    /// Required to execute, `remove` or `edit`
    #[clap(long, short, action, default_value = "false")]
    modify: bool,
    /// Config table to update
    ///
    /// **Note**: The table will be created if it does not currently exist and if there are no existing values that share the same key
    #[clap(short, long, default_value = "")]
    table: String,
    /// Item type to write into config
    ///
    /// Default to `string`
    #[clap(short = 'X', long, value_enum, default_value = "str")]
    value_type: Types,
    /// Extended item type to write to config
    ///
    /// Allows for complex expressions such as,
    ///
    /// `-X str -Y append`
    #[clap(short = 'Y', long, value_enum)]
    extended_value_type: Option<Types>,
    /// Config key name
    ///
    /// Restrictions: MUST be a valid TOML table key
    key: String,
    /// Value for the config
    ///
    /// Restrictions: MUST be a valid TOML table value
    value: Option<toml_edit::Item>,
    /// Import path that was set when value_type was -X import
    #[clap(skip)]
    import_path: Option<PathBuf>,
}

/// Enumeration of table actions that can be evaluated by table-args
#[derive(Subcommand, Debug, Clone)]
pub enum TableAction {
    /// Inserts a value for a key to a table
    Insert(TableArgs),
    /// Replaces the value of a key from a table
    Replace(TableArgs),
    /// Removes a key from a table
    Remove(TableArgs),
    /// Views the value of a key from a table
    View(TableArgs),
    /// Checks if a key exists in a table
    Exists(TableArgs),
    /// Indicates that the remove would have been successful
    #[clap(skip)]
    WouldRemove,
    /// Indicates that the replace would have been successful
    #[clap(skip)]
    WouldReplace,
    /// Indicates the action would have rejected due to a value type mismatch
    #[clap(skip)]
    RejectTypeMismatch,
    /// Indicates the action would have been rejected due to an existing value not matching the expected value
    #[clap(skip)]
    RejectExistingValueMismatch,
    /// (internal) Indicates the action would have been rejected because the corresponding table is missing
    #[clap(skip)]
    InternalRejectMissingTable,
    /// Indicates no action would have been evaluated
    #[clap(skip)]
    NoOP,
}

impl TableArgs {
    /// Returns the action the current set of args would apply to a document
    #[inline]
    pub fn action(&self, doc: &DocumentMut) -> Option<TableAction> {
        if !self.has_table(doc) {
            return Some(TableAction::InternalRejectMissingTable);
        }

        match self {
            Self {
                remove: true,
                modify: true,
                value_type,
                ..
            } => self.get_entry(doc).map(|e| {
                if value_type.is_type(e) {
                    TableAction::Replace(self.clone())
                } else {
                    TableAction::RejectTypeMismatch
                }
            }),
            Self {
                remove: false,
                modify: true,
                value_type,
                value,
                ..
            } => {
                let item = value.as_ref().unwrap_or(&toml_edit::Item::None);
                self.get_entry(doc).map(|e| {
                    if value_type.is_type(e) {
                        if item.is_none() {
                            TableAction::View(self.clone())
                        } else if item.to_string() == e.to_string() {
                            TableAction::Replace(self.clone())
                        } else {
                            TableAction::RejectExistingValueMismatch
                        }
                    } else {
                        TableAction::RejectTypeMismatch
                    }
                })
            }
            Self {
                remove: true,
                modify: false,
                value_type,
                ..
            } => self.get_entry(doc).map(|e| {
                if value_type.is_type(e) {
                    TableAction::WouldRemove
                } else {
                    TableAction::RejectTypeMismatch
                }
            }),
            Self {
                remove: false,
                modify: false,
                value,
                value_type,
                ..
            } => {
                let item = value.as_ref().unwrap_or(&toml_edit::Item::None);
                Some(
                    self.get_entry(doc)
                        .map(|e| {
                            if value_type.is_type(e) {
                                if item.is_none() {
                                    TableAction::NoOP
                                } else if item.to_string() == e.to_string() {
                                    TableAction::Exists(self.clone())
                                } else {
                                    TableAction::RejectExistingValueMismatch
                                }
                            } else {
                                TableAction::RejectTypeMismatch
                            }
                        })
                        .unwrap_or(TableAction::Insert(self.clone())),
                )
            }
        }
    }

    /// Evaluates the current action w/ this
    pub fn eval(&self, doc: &mut DocumentMut) -> Result<TableAction> {
        let mut action = self.action(&doc);

        if let Some(TableAction::InternalRejectMissingTable) = action {
            self.get_table_mut(doc)?;
            action = self.action(&doc);
        }

        if let Some(action) = action {
            match action {
                TableAction::Remove(..) => {
                    self.remove_item(doc)?;
                }
                TableAction::Replace(..) | TableAction::Insert(..) => {
                    self.set_item(doc)?;
                }
                TableAction::View(..) => {
                    if let Some(entry) = self.get_entry(doc) {
                        println!("{}", entry.to_string());
                    }
                }
                TableAction::Exists(..) => {}
                _ => {}
            }
            Ok(action)
        } else {
            Err(anyhow!("Could not evaluate table action from arguments"))
        }
    }
    /// Returns a mutable reference to the table specified by this configuration
    #[inline]
    pub fn get_table_mut<'a: 'b, 'b>(&self, doc: &'a mut DocumentMut) -> Result<&'b mut Table> {
        self.table
            .as_str()
            .split('.')
            .fold(Ok(doc.as_table_mut()), |t, k| {
                let t = t?;
                if !t.contains_table(k) && !t.contains_key(k) {
                    t[k] = toml_edit::table();
                    Ok(t[k].as_table_mut().expect("should exist just added"))
                } else if t.contains_table(k) {
                    Ok(t[k].as_table_mut().expect("should exist just checked"))
                } else {
                    Err(anyhow!("Could not create table"))
                }
            })
    }

    /// Get a reference to the table specified by this configuration
    #[inline]
    pub fn get_table<'a: 'b, 'b>(&self, doc: &'a DocumentMut) -> Option<&'b Table> {
        self.table
            .split(['.'])
            .try_fold(doc.as_table(), |t, k| t.get(k).and_then(|v| v.as_table()))
    }

    /// Returns true if the document has the table specified by this configuration
    #[inline]
    pub fn has_table<'a: 'b, 'b>(&self, doc: &DocumentMut) -> bool {
        self.get_table(doc).is_some()
    }

    /// Fetches an entry from the document
    pub fn get_entry<'a: 'b, 'b>(&self, doc: &'a DocumentMut) -> Option<&'b toml_edit::Item> {
        self.get_table(doc)?.get(&self.key)
    }

    /// Write a value to a document
    ///
    /// Returns the previous value if a previous value exists
    ///
    /// **Errors**:
    /// - Returns an error if the requested table was unable to be created
    /// - Returns an error if the table entry was occupied, and the existing value did not match the expected type
    pub fn set_item(&self, doc: &mut DocumentMut) -> Result<Option<toml_edit::Item>> {
        if let Some(item) = self.value.as_ref().cloned()
        // .map(|v| self.value_type.transmute_item(v.clone()))
        {
            let table = self.get_table_mut(doc)?;
            match table.entry(&self.key) {
                toml_edit::Entry::Occupied(mut occupied) => {
                    let is_expected_ty = match self.value_type {
                        Types::String => occupied.get().is_str(),
                        Types::Bool => occupied.get().is_bool(),
                        Types::Float => occupied.get().is_float(),
                        Types::Integer => occupied.get().is_integer(),
                        Types::Object => occupied.get().is_inline_table(),
                        Types::Append => occupied.get().is_array(),
                        Types::Import => occupied.get().is_table(),
                    };
                    if is_expected_ty {
                        let replaced = occupied.insert(item);
                        Ok(Some(replaced))
                    } else {
                        Err(anyhow!("Could not set item"))
                    }
                }
                toml_edit::Entry::Vacant(vacant) => {
                    vacant.insert(item);
                    Ok(None)
                }
            }
        } else {
            Err(anyhow!("Could not set item"))
        }
    }

    /// Removes an item from a document
    pub fn remove_item(&self, doc: &mut DocumentMut) -> Result<Option<toml_edit::Item>> {
        if let Some(item) = self.value.as_ref().cloned()
        // .map(|v| self.value_type.transmute_item(v.clone()))
        {
            let table = self.get_table_mut(doc)?;
            let can_remove = match table.entry(&self.key) {
                toml_edit::Entry::Occupied(occupied) => {
                    let is_expected_ty = match self.value_type {
                        Types::String => occupied.get().is_str(),
                        Types::Bool => occupied.get().is_bool(),
                        Types::Float => occupied.get().is_float(),
                        Types::Integer => occupied.get().is_integer(),
                        Types::Object => occupied.get().is_inline_table(),
                        Types::Append => occupied.get().is_array(),
                        Types::Import => occupied.get().is_table(),
                    };
                    is_expected_ty && occupied.get().to_string() == item.to_string()
                }
                toml_edit::Entry::Vacant(_) => false,
            };

            if can_remove {
                Ok(table.remove(&self.key))
            } else {
                Err(anyhow!("Cannot remove item"))
            }
        } else {
            let table = self.get_table_mut(doc)?;
            Ok(table.remove(&self.key))
        }
    }

    /// Returns a view of the item stored in the document
    pub fn view_item<'a: 'b, 'b>(&self, doc: &'a DocumentMut) -> Option<&'b toml_edit::Item> {
        let table = self.get_table(doc)?;
        table.get(&self.key).and_then(|t| {
            if self.value_type.is_type(t) {
                if let Some(value) = self.value.as_ref() {
                    if value.to_string() == t.to_string() {
                        Some(t)
                    } else {
                        None
                    }
                } else {
                    Some(t)
                }
            } else {
                None
            }
        })
    }

    /// Sets the key/value/value_ty settings
    #[inline]
    pub fn set_kvp<KVP: KeyValueType>(&mut self, key: &str, value: KVP) {
        self.set_key(key);
        value.configure_args(self);

        if matches!(value.db_type(), Types::Import) {}
    }

    /// Sets the current table
    ///
    /// Returns the last table setting if set
    #[inline]
    pub fn set_table(&mut self, table: impl Into<String>) {
        self.table = table.into();
    }

    /// Sets the current value ty
    #[inline]
    pub fn set_value_ty(&mut self, ty: Types) {
        self.value_type = ty;
    }

    /// Sets the current key
    #[inline]
    pub fn set_key(&mut self, key: &str) {
        self.key = key.to_string();
    }

    /// Sets the current value as item
    #[inline]
    pub fn set_value(&mut self, item: toml_edit::Item) {
        self.value = Some(item);
    }

    /// Sets the value of the modify flag
    #[inline]
    pub fn set_modify(&mut self, modify: bool) {
        self.modify = modify;
    }

    /// Sets the value of the remove flag
    #[inline]
    pub fn set_remove(&mut self, remove: bool) {
        self.remove = remove;
    }
}

impl Display for TableAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableAction::Insert(args) => {
                write!(f, "insert {args}")
            }
            TableAction::Replace(args) => {
                write!(f, "replace {args}")
            }
            TableAction::Remove(args) => {
                write!(f, "remove {args}")
            }
            TableAction::View(args) => {
                write!(f, "view {args}")
            }
            TableAction::Exists(args) => {
                write!(f, "exists {args}")
            }
            _ => Ok(()), // TableAction::TableCreated => todo!(),
                         // TableAction::WouldRemove => todo!(),
                         // TableAction::WouldReplace => todo!(),
                         // TableAction::RejectTypeMismatch => todo!(),
                         // TableAction::RejectExistingValueMismatch => todo!(),
                         // TableAction::RejectMissingTable => todo!(),
                         // TableAction::NoOP => todo!(),
        }
    }
}

impl Display for TableArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.modify {
            write!(f, "--modify ")?;
        }

        if self.remove {
            write!(f, "--remove ")?;
        }

        write!(f, "-t '{}' ", self.table)?;
        write!(f, "-X {} ", self.value_type)?;
        if let Some(y) = self.extended_value_type {
            write!(f, "-Y {y} ")?;
        }
        write!(f, "'{}'", self.key)?;

        // Special Case: If -X import is used, than internally import_path is likely set
        if let Some(import_path) = self
            .import_path
            .as_ref()
            .filter(|_| matches!(self.value_type, Types::Import))
        {
            write!(f, " -- '{}'", import_path.to_string_lossy())?;
        } else if let Some(value) = self.value.as_ref() {
            write!(f, " -- {value}")?;
        }

        Ok(())
    }
}

pub trait KeyValueType {
    fn configure_args(&self, args: &mut TableArgs) {
        args.set_value(self.to_toml_item());
        args.set_value_ty(self.db_type());
    }

    fn db_type(&self) -> Types;

    fn to_toml_item(&self) -> toml_edit::Item {
        toml_edit::Item::None
    }
}

impl KeyValueType for PathBuf {
    fn configure_args(&self, args: &mut TableArgs) {
        args.set_value(self.to_toml_item());
        args.set_value_ty(self.db_type());
        args.import_path = Some(self.clone());
    }

    fn db_type(&self) -> Types {
        Types::Import
    }

    fn to_toml_item(&self) -> toml_edit::Item {
        match std::fs::read_to_string(self)
            .and_then(|r| Ok(toml_edit::ImDocument::parse(r).unwrap()))
        {
            Ok(doc) => doc.as_item().clone(),
            Err(_) => toml_edit::Item::None,
        }
    }
}

impl KeyValueType for toml_edit::Item {
    fn to_toml_item(&self) -> toml_edit::Item {
        self.clone()
    }

    fn db_type(&self) -> Types {
        match self {
            toml_edit::Item::None => Types::String,
            toml_edit::Item::Value(v) => match v {
                toml_edit::Value::String(_) => Types::String,
                toml_edit::Value::Integer(_) => Types::Integer,
                toml_edit::Value::Float(_) => Types::Float,
                toml_edit::Value::Boolean(_) => Types::Bool,
                toml_edit::Value::Datetime(_) => Types::String,
                toml_edit::Value::Array(_) => Types::Append,
                toml_edit::Value::InlineTable(_) => Types::Object,
            },
            toml_edit::Item::Table(_) => Types::Object,
            toml_edit::Item::ArrayOfTables(_) => Types::Object,
        }
    }
}

impl KeyValueType for String {
    fn to_toml_item(&self) -> toml_edit::Item {
        value(self)
    }

    fn db_type(&self) -> Types {
        Types::String
    }
}

impl<'a> KeyValueType for &'a str {
    fn to_toml_item(&self) -> toml_edit::Item {
        value(*self)
    }

    fn db_type(&self) -> Types {
        Types::String
    }
}

impl KeyValueType for f64 {
    fn to_toml_item(&self) -> toml_edit::Item {
        value(*self)
    }

    fn db_type(&self) -> Types {
        Types::Float
    }
}

impl KeyValueType for f32 {
    fn to_toml_item(&self) -> toml_edit::Item {
        value(*self as f64)
    }

    fn db_type(&self) -> Types {
        Types::Float
    }
}

impl KeyValueType for usize {
    fn to_toml_item(&self) -> toml_edit::Item {
        value(*self as i64)
    }

    fn db_type(&self) -> Types {
        Types::Integer
    }
}

impl KeyValueType for u64 {
    fn to_toml_item(&self) -> toml_edit::Item {
        value(*self as i64)
    }

    fn db_type(&self) -> Types {
        Types::Integer
    }
}

impl<'a> KeyValueType for u32 {
    fn to_toml_item(&self) -> toml_edit::Item {
        value(*self as i64)
    }

    fn db_type(&self) -> Types {
        Types::Integer
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::split_args;
    use std::str::FromStr;

    use super::TableAction;
    use clap::Parser;

    #[derive(Parser)]
    struct Test {
        #[clap(subcommand)]
        command: TableAction,
    }

    #[test]
    fn test_cli_args() {
        let args = split_args("test insert --table 'test' key -- \'value\'").unwrap();
        let command = Test::parse_from(args);

        assert!(matches!(command.command, TableAction::Insert(..)));
        if let TableAction::Insert(args) = command.command {
            assert!(!args.remove);
            assert!(!args.modify);
            assert_eq!("test", args.table.as_str());
            assert_eq!("key", args.key.as_str());
            assert_eq!("value", args.value.unwrap().as_str().unwrap());
        }
    }
}
