use std::fmt::Display;
use std::str::FromStr;

use crate::Result;
use crate::Types;
use anyhow::anyhow;
use clap::clap_derive::*;
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
    #[clap(short = 'x', long, value_enum, default_value = "str")]
    value_type: Types,
    /// Config key name
    ///
    /// Restrictions: MUST be a valid TOML table key
    key: String,
    /// Value for the config
    ///
    /// Restrictions: MUST be a valid TOML table value
    value: Option<toml_edit::Value>,
}

#[derive(Debug, Clone)]
pub enum TableAction {
    TableCreated,
    Remove,
    WouldRemove,
    Replace,
    WouldReplace,
    Insert,
    View,
    Exists,
    RejectTypeMismatch,
    RejectExistingValueMismatch,
    RejectMissingTable,
    NoOP,
}

impl TableArgs {
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

    /// Returns the action the current set of args would take on the document
    pub fn action(&self, doc: &DocumentMut) -> Option<TableAction> {
        if !self.has_table(doc) {
            return Some(TableAction::RejectMissingTable);
        }

        match self {
            Self {
                remove: true,
                modify: true,
                value_type,
                ..
            } => self.get_entry(doc).map(|e| {
                if value_type.is_type(e) {
                    TableAction::Replace
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
                let item = value
                    .as_ref()
                    .map(|v| toml_edit::value(v))
                    .unwrap_or(toml_edit::Item::None);
                self.get_entry(doc).map(|e| {
                    if value_type.is_type(e) {
                        if item.is_none() {
                            TableAction::View
                        } else if item.to_string() == e.to_string() {
                            TableAction::Replace
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
                let item = value
                    .as_ref()
                    .map(|v| toml_edit::value(v))
                    .unwrap_or(toml_edit::Item::None);
                Some(
                    self.get_entry(doc)
                        .map(|e| {
                            if value_type.is_type(e) {
                                if item.is_none() {
                                    TableAction::NoOP
                                } else if item.to_string() == e.to_string() {
                                    TableAction::Exists
                                } else {
                                    TableAction::RejectExistingValueMismatch
                                }
                            } else {
                                TableAction::RejectTypeMismatch
                            }
                        })
                        .unwrap_or(TableAction::Insert),
                )
            }
        }
    }

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
        if let Some(Ok(item)) = self
            .value
            .as_ref()
            .map(|v| self.value_type.transmute_item(v.clone()))
        {
            let table = self.get_table_mut(doc)?;
            match table.entry(&self.key) {
                toml_edit::Entry::Occupied(mut occupied) => {
                    let is_expected_ty = match self.value_type {
                        Types::String => occupied.get().is_str(),
                        Types::Bool => occupied.get().is_bool(),
                        Types::Float => occupied.get().is_float(),
                        Types::Integer => occupied.get().is_integer(),
                        Types::InlineTable => occupied.get().is_inline_table(),
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

    pub fn remove_item(&self, doc: &mut DocumentMut) -> Result<Option<toml_edit::Item>> {
        if let Some(Ok(item)) = self
            .value
            .as_ref()
            .map(|v| self.value_type.transmute_item(v.clone()))
        {
            let table = self.get_table_mut(doc)?;
            let can_remove = match table.entry(&self.key) {
                toml_edit::Entry::Occupied(occupied) => {
                    let is_expected_ty = match self.value_type {
                        Types::String => occupied.get().is_str(),
                        Types::Bool => occupied.get().is_bool(),
                        Types::Float => occupied.get().is_float(),
                        Types::Integer => occupied.get().is_integer(),
                        Types::InlineTable => occupied.get().is_inline_table(),
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
                if let Some(value) = self.value.as_ref().map(|v| toml_edit::value(v)) {
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

    /// Evaluates the current action w/ this
    pub fn eval(&self, doc: &mut DocumentMut) -> Result<TableAction> {
        let mut action = self.action(&doc);

        if let Some(TableAction::RejectMissingTable) = action {
            // TODO -- Log that a table was created
            {
                self.get_table_mut(doc)?;
            }
            action = self.action(&doc);
        }

        if let Some(action) = action {
            match action {
                TableAction::Remove => {
                    self.remove_item(doc)?;
                }
                TableAction::Replace | TableAction::Insert => {
                    self.set_item(doc)?;
                }
                TableAction::View => {
                    if let Some(entry) = self.get_entry(doc) {
                        println!("{}", entry.to_string());
                    }
                }
                TableAction::Exists => {}
                _ => {}
            }
            Ok(action)
        } else {
            Err(anyhow!("Could not evaluate table action from arguments"))
        }
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

    /// Sets the current value
    #[inline]
    pub fn set_value(&mut self, value: &str) -> Result<()> {
        self.value = Some(toml_edit::Value::from_str(value)?);
        Ok(())
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

impl Display for TableArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.modify {
            write!(f, "--modify ")?;
        }

        if self.remove {
            write!(f, "--remove ")?;
        }

        write!(f, "-t '{}' ", self.table)?;
        write!(f, "-x {} ", self.value_type)?;
        write!(f, "'{}'", self.key)?;

        if let Some(value) = self.value.as_ref() {
            write!(f, " -- {value}")?;
        }
        Ok(())
    }
}
