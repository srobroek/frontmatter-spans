//! The parsed value tree. Every node and every key carries a [`Span`].

use crate::Span;

/// A parsed value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// YAML null, or an empty block.
    Null,
    /// A boolean.
    Bool(bool),
    /// An integer that fits `i64`.
    Integer(i64),
    /// A floating-point number.
    Float(f64),
    /// A string.
    String(String),
    /// A TOML date or time, in its TOML text form.
    Datetime(String),
    /// A sequence.
    Array(Vec<Node>),
    /// A mapping, in document order. Duplicate keys can occur in JSON.
    Table(Vec<Entry>),
}

/// A value and where it sits.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The value.
    pub value: Value,
    /// The source range of the value as written. Quoted scalars include the quotes.
    ///
    /// YAML mappings and sequences are the exception: their span is zero-width at the start
    /// of their first entry, so use the children's spans for extents.
    pub span: Span,
}

/// One key/value pair of a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The key.
    pub key: String,
    /// The source range of the key as written.
    pub key_span: Span,
    /// The value.
    pub value: Node,
}

impl Node {
    /// The entry for `key` when this node is a table. The last duplicate wins.
    #[must_use]
    pub fn entry(&self, key: &str) -> Option<&Entry> {
        match &self.value {
            Value::Table(entries) => entries.iter().rev().find(|e| e.key == key),
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Datetime(_)
            | Value::Array(_) => None,
        }
    }

    /// The value for `key` when this node is a table.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entry(key).map(|e| &e.value)
    }

    /// The string when this node is a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::String(s) => Some(s),
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::Datetime(_)
            | Value::Array(_)
            | Value::Table(_) => None,
        }
    }
}
