//! Resource limits for parsing untrusted front matter.

/// Caps that keep hostile input from exhausting the stack or memory.
///
/// Exceeding a limit is a [`ParseError`](crate::ParseError), never an abort. Start from
/// [`Limits::default`] and change what you need. The struct is `#[non_exhaustive]` so that new
/// limits are not a breaking change.
///
/// ```
/// use frontmatter_spans::Limits;
///
/// let strict = Limits::default().with_max_depth(16);
/// assert_eq!(strict.max_depth, 16);
/// assert_eq!(strict.max_input_bytes, 16 << 20);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// Deepest nesting of tables and arrays, counting the root as 1. Default 128.
    ///
    /// Applies to YAML and JSON. TOML relies on the `toml_edit` nesting limit.
    pub max_depth: usize,
    /// Largest block content to parse, in bytes. Default 16 MiB.
    pub max_input_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 128,
            max_input_bytes: 16 << 20,
        }
    }
}

impl Limits {
    /// Set [`max_depth`](Self::max_depth).
    #[must_use]
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set [`max_input_bytes`](Self::max_input_bytes).
    #[must_use]
    pub fn with_max_input_bytes(mut self, max_input_bytes: usize) -> Self {
        self.max_input_bytes = max_input_bytes;
        self
    }
}
