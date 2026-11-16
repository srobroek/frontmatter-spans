//! Byte ranges and display positions over a whole document.

use std::ops::Range;

/// A 1-based line and column.
///
/// The column counts Unicode code points from the start of the line, plus one. A leading
/// byte-order mark is not part of line 1, so the first character after it is column 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineCol {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column in code points.
    pub column: usize,
}

/// A half-open byte range of the whole document, with its line and column ends.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Span {
    /// Byte range `[start, end)`. Indexes the whole document, not the front-matter block.
    pub range: Range<usize>,
    /// Position of `range.start`.
    pub start: LineCol,
    /// Position of `range.end`.
    pub end: LineCol,
}

/// Maps byte offsets of one document to lines and columns.
///
/// Lines end at LF, CRLF or a lone CR. Nothing else ends a line. Building the index is linear
/// in the document, and each lookup is logarithmic: a column is the byte distance from the
/// line start minus the extra bytes of the multi-byte characters in between.
#[derive(Debug, Clone)]
pub struct LineIndex<'a> {
    text: &'a str,
    starts: Vec<usize>,
    /// `(byte offset, extra bytes before and including it)` for each multi-byte character.
    wide: Vec<(usize, usize)>,
}

impl<'a> LineIndex<'a> {
    /// Index `text`, which must be the whole document.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        let bom = if text.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        let bytes = text.as_bytes();
        let mut starts = vec![bom];
        for (i, b) in bytes.iter().enumerate() {
            match b {
                b'\n' => starts.push(i + 1),
                b'\r' if bytes.get(i + 1) != Some(&b'\n') => starts.push(i + 1),
                _ => {}
            }
        }
        let mut wide = Vec::new();
        let mut extra = 0;
        if !text.is_ascii() {
            for (i, c) in text.char_indices() {
                if c.len_utf8() > 1 {
                    extra += c.len_utf8() - 1;
                    wide.push((i, extra));
                }
            }
        }
        Self { text, starts, wide }
    }

    /// Extra bytes (beyond one per character) in `text[..offset]`.
    fn extra_before(&self, offset: usize) -> usize {
        self.wide
            .partition_point(|&(at, _)| at < offset)
            .checked_sub(1)
            .and_then(|i| self.wide.get(i))
            .map_or(0, |&(_, extra)| extra)
    }

    /// The line and column of byte `offset`.
    ///
    /// An offset past the end clamps to the end. An offset inside a character moves back to
    /// the start of that character, and one inside the byte-order mark counts as line 1,
    /// column 1.
    #[must_use]
    pub fn line_col(&self, offset: usize) -> LineCol {
        let mut offset = offset
            .min(self.text.len())
            .max(self.starts.first().copied().unwrap_or(0));
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.starts.partition_point(|&s| s <= offset) - 1;
        let start = self.starts.get(line).copied().unwrap_or(0);
        let column = offset - start - (self.extra_before(offset) - self.extra_before(start)) + 1;
        LineCol {
            line: line + 1,
            column,
        }
    }

    /// The [`Span`] for a byte range.
    #[must_use]
    pub fn span(&self, range: Range<usize>) -> Span {
        Span {
            start: self.line_col(range.start),
            end: self.line_col(range.end),
            range,
        }
    }
}
