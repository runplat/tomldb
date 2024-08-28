use std::fmt::Display;
use clap::clap_derive::*;

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
    Object,
    /// Parses the value as valid TOML and appends to an existing value
    /// 
    /// If used as the main value type (`-X`) will assume the item is a string
    #[clap(name = "append")]
    Append,
    /// Parses the value as valid TOML string and interprets the string
    /// as a path to import a TOML document from
    #[clap(name = "import")]
    Import,
}

impl Types {
    /// Returns true if the item matches this type
    pub fn is_type(&self, value: &toml_edit::Item) -> bool {
        match self {
            Types::String => value.as_str().is_some(),
            Types::Bool => value.as_bool().is_some(),
            Types::Float => value.as_float().is_some(),
            Types::Integer => value.as_integer().is_some(),
            Types::Object => value.as_inline_table().is_some(),
            Types::Append => value.as_array().is_some(),
            Types::Import => value.as_table().is_some(),
        }
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
            Types::Object => {
                write!(f, "obj")
            },
            Types::Append => {
                write!(f, "append")
            },
            Types::Import => write!(f, "import"),
        }
    }
}