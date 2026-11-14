//! Error types.

use crate::{Format, Span};

/// The front-matter block does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {format} front matter: {message}")]
pub struct ParseError {
    /// The syntax that failed.
    pub format: Format,
    /// What the parser reported.
    pub message: String,
    /// Where it stopped, in whole-document coordinates, when the parser says.
    pub span: Option<Span>,
}

/// A value does not fit the type being deserialised.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct DeError {
    /// What went wrong.
    pub message: String,
    /// The key or value at fault, in whole-document coordinates.
    pub span: Option<Span>,
}

impl serde::de::Error for DeError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Self {
            message: msg.to_string(),
            span: None,
        }
    }
}

impl DeError {
    pub(crate) fn or_span(mut self, span: &Span) -> Self {
        if self.span.is_none() {
            self.span = Some(span.clone());
        }
        self
    }
}
