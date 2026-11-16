#![doc = include_str!("../README.md")]
// Restriction lints guard library code against panics and silent failure. They live here,
// not in `[lints]`, so the integration tests keep to the pedantic set.
#![warn(
    clippy::unwrap_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::dbg_macro,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::exit,
    clippy::mem_forget,
    clippy::get_unwrap,
    clippy::unwrap_in_result,
    clippy::clone_on_ref_ptr,
    clippy::redundant_type_annotations,
    clippy::redundant_clone,
    clippy::use_debug,
    clippy::string_add,
    clippy::needless_pass_by_ref_mut,
    clippy::expect_used,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::let_underscore_must_use,
    clippy::map_err_ignore,
    clippy::str_to_string,
    clippy::wildcard_enum_match_arm,
    clippy::as_conversions,
    clippy::significant_drop_tightening,
    clippy::option_if_let_else,
    clippy::indexing_slicing,
    clippy::string_slice
)]

mod de;
mod detect;
mod error;
mod json;
mod limits;
mod span;
mod toml;
mod value;
mod yaml;

use std::fmt;

use serde::Deserialize;

pub use detect::{Block, detect};
pub use error::{DeError, ParseError};
pub use limits::Limits;
pub use span::{LineCol, LineIndex, Span};
pub use value::{Entry, Node, Value};

/// The syntax of a front-matter block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// Between `---` lines.
    Yaml,
    /// Between `+++` lines.
    Toml,
    /// A JSON object at the start of the document.
    Json,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Json => "JSON",
        })
    }
}

/// A located and parsed front-matter block.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontMatter<'a> {
    /// Where the block sits.
    pub block: Block,
    /// The raw text between the delimiters. For JSON, the whole object.
    pub raw: &'a str,
    /// The parsed value. Usually a table; YAML can also give a scalar, array or null.
    pub root: Node,
}

impl<'a> FrontMatter<'a> {
    /// Find and parse the front matter at the start of `doc`, with the default [`Limits`].
    ///
    /// Returns `Ok(None)` when the document has none.
    pub fn parse(doc: &'a str) -> Result<Option<Self>, ParseError> {
        Self::parse_with(doc, &Limits::default())
    }

    /// Like [`parse`](Self::parse), with explicit [`Limits`].
    pub fn parse_with(doc: &'a str, limits: &Limits) -> Result<Option<Self>, ParseError> {
        let Some(block) = detect(doc) else {
            return Ok(None);
        };
        Self::parse_block_with(doc, block, limits).map(Some)
    }

    /// Parse a block that [`fn@detect`] found in `doc`, with the default [`Limits`].
    ///
    /// `block` must come from `detect(doc)` for this same `doc`; otherwise the spans are
    /// meaningless.
    pub fn parse_block(doc: &'a str, block: Block) -> Result<Self, ParseError> {
        Self::parse_block_with(doc, block, &Limits::default())
    }

    /// Like [`parse_block`](Self::parse_block), with explicit [`Limits`].
    pub fn parse_block_with(
        doc: &'a str,
        block: Block,
        limits: &Limits,
    ) -> Result<Self, ParseError> {
        let Some(raw) = doc.get(block.content.clone()) else {
            return Err(ParseError {
                format: block.format,
                message: "the block range is not valid text of the document".to_owned(),
                span: None,
            });
        };
        if raw.len() > limits.max_input_bytes {
            return Err(ParseError {
                format: block.format,
                message: format!(
                    "the block is larger than the limit of {} bytes",
                    limits.max_input_bytes
                ),
                span: None,
            });
        }
        let index = LineIndex::new(doc);
        let base = block.content.start;
        let root = match block.format {
            Format::Yaml => yaml::parse(raw, base, &index, limits)?,
            Format::Toml => toml::parse(raw, base, &index)?,
            Format::Json => json::parse(raw, base, &index, limits)?,
        };
        Ok(Self { block, raw, root })
    }

    /// The value for a top-level `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.root.get(key)
    }

    /// Deserialise the whole block into `T`.
    pub fn deserialize<'de, T: Deserialize<'de>>(&'de self) -> Result<T, DeError> {
        self.root.deserialize()
    }

    /// Deserialise the top-level `key` into `T`, with the span of its value.
    ///
    /// Returns `Ok(None)` when the key is absent.
    pub fn deserialize_key<'de, T: Deserialize<'de>>(
        &'de self,
        key: &str,
    ) -> Result<Option<Spanned<T>>, DeError> {
        self.get(key)
            .map(|node| {
                Ok(Spanned {
                    value: node.deserialize()?,
                    span: node.span.clone(),
                })
            })
            .transpose()
    }
}

/// A deserialised value and the span of the source it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    /// The value.
    pub value: T,
    /// Its source range.
    pub span: Span,
}

impl Node {
    /// Deserialise this node into `T`.
    pub fn deserialize<'de, T: Deserialize<'de>>(&'de self) -> Result<T, DeError> {
        T::deserialize(de::NodeDe(self)).map_err(|e| e.or_span(&self.span))
    }
}
