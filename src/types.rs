use std::{fmt::Display, str::FromStr};

use clap::clap_derive::*;
use crate::Result;

/// Enum containing variants of supported TOML value types
#[derive(Default, Copy, Clone, Debug, ValueEnum)]
pub enum Types {
    /// Parses the value as valid TOML string
    #[clap(name = "str")]
    #[default]
    String,
    /// Parses the value as a valid TOML bool
    #[clap(name = "bool")]
    Bool,
    /// Parses the value as a valid TOML float
    #[clap(name = "float")]
    Float,
    /// Parses the value as a valid TOML integer
    #[clap(name = "int")]
    Integer,
    /// Parses the value as a valid TOML inline table
    #[clap(name = "obj")]
    InlineTable,
}

impl Types {
    /// Returns true if the item matches this type
    pub fn is_type(&self, value: &toml_edit::Item) -> bool {
        match self {
            Types::String => value.as_str().is_some(),
            Types::Bool => value.as_bool().is_some(),
            Types::Float => value.as_float().is_some(),
            Types::Integer => value.as_integer().is_some(),
            Types::InlineTable => value.as_inline_table().is_some(),
        }
    }

    /// Transmutes a toml_edit::Value into a toml_edit::Item
    pub fn transmute_item(&self, value: toml_edit::Value) -> Result<toml_edit::Item> {
        // Parse the value **ONLY** after unlocking the locked file in case there is a parse error
        let item = match self {
            Types::String => toml_edit::value(value),
            Types::Bool => {
                toml_edit::value(value.as_bool().expect("should be a valid bool value"))
            }
            Types::Float => {
                toml_edit::value(value.as_float().expect("should be a valid float value"))
            }
            Types::Integer => {
                toml_edit::value(value.as_integer().expect("should be a valid float value"))
            }
            Types::InlineTable => {
                let value = value.to_string().replace(r#"\""#, "\"");
                let value = value.trim_matches(['\'']);
                let intermediate = format!(
                    r"
            val = {value}
            "
                );
                let doc = toml_edit::ImDocument::from_str(intermediate.as_str())?;
                toml_edit::value(
                    doc["val"]
                        .as_inline_table()
                        .cloned()
                        .expect("should be valid inline table"),
                )
            }
        };

        Ok(item)
    }

    /// Parses an item from an input str
    pub fn parse_item(&self, input: &str) -> Result<toml_edit::Item> {
        let value = toml_edit::Value::from_str(input)?;
        self.transmute_item(value)
    }
}

impl Display for Types {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::String => {
                write!(f, "str")
            },
            Types::Bool => {
                write!(f, "bool")
            },
            Types::Float => {
                write!(f, "float")
            },
            Types::Integer => {
                write!(f, "int")
            },
            Types::InlineTable => {
                write!(f, "obj")
            },
        }
    }
}