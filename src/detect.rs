//! Finding the front-matter block at the start of a document.

use std::ops::Range;

use crate::Format;

/// Where a front-matter block sits in a document. Every offset indexes the whole document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The syntax, from the opening delimiter.
    pub format: Format,
    /// From the opening delimiter to the end of the closing one, terminator excluded.
    pub outer: Range<usize>,
    /// The text between the delimiter lines. For JSON this is the whole object.
    pub content: Range<usize>,
    /// Where the body starts: after the closing delimiter's line terminator, if it has one.
    pub body_start: usize,
}

/// Find the front-matter block at the start of `doc`.
///
/// A leading UTF-8 byte-order mark is skipped. LF, CRLF and lone CR all end a delimiter line.
/// A block that never closes is not front matter.
#[must_use]
pub fn detect(doc: &str) -> Option<Block> {
    let rest = doc.strip_prefix('\u{feff}').unwrap_or(doc);
    let start = doc.len().saturating_sub(rest.len());
    if rest.starts_with('{') {
        return json(doc, start);
    }
    let (format, fence) = if rest.starts_with("---") {
        (Format::Yaml, "---")
    } else if rest.starts_with("+++") {
        (Format::Toml, "+++")
    } else {
        return None;
    };
    let (open_end, content_start) = line_at(doc, start);
    if open_end == content_start || !is_fence(text(doc, start..open_end), fence, false) {
        return None;
    }
    let mut pos = content_start;
    while pos < doc.len() {
        let (end, next) = line_at(doc, pos);
        if is_fence(text(doc, pos..end), fence, format == Format::Yaml) {
            return Some(Block {
                format,
                outer: start..end,
                content: content_start..pos,
                body_start: next,
            });
        }
        pos = next;
    }
    None
}

/// The text of `range`, or empty when the range is not a valid slice of `doc`.
///
/// Every range here ends at an ASCII line terminator or a brace, so the empty case never occurs.
fn text(doc: &str, range: Range<usize>) -> &str {
    doc.get(range).unwrap_or_default()
}

/// `(end excluding terminator, start of next line)` for the line beginning at `pos`.
fn line_at(doc: &str, pos: usize) -> (usize, usize) {
    let bytes = doc.as_bytes();
    let Some(rel) = bytes
        .get(pos..)
        .and_then(|tail| tail.iter().position(|b| matches!(b, b'\n' | b'\r')))
    else {
        return (doc.len(), doc.len());
    };
    let end = pos + rel;
    let crlf = bytes.get(end) == Some(&b'\r') && bytes.get(end + 1) == Some(&b'\n');
    (end, end + if crlf { 2 } else { 1 })
}

/// A delimiter line is the fence, or `...` for YAML, plus optional trailing blanks.
fn is_fence(line: &str, fence: &str, yaml_end: bool) -> bool {
    let t = line.trim_end_matches([' ', '\t']);
    t == fence || (yaml_end && t == "...")
}

/// Find the end of the leading JSON object by matching braces outside strings.
fn json(doc: &str, start: usize) -> Option<Block> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in doc.get(start..)?.char_indices() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = start + i + 1;
                    return Some(Block {
                        format: Format::Json,
                        outer: start..end,
                        content: start..end,
                        body_start: json_body_start(doc, end),
                    });
                }
            }
            _ => {}
        }
    }
    None
}

/// The body starts on the next line when only blanks follow the closing brace.
fn json_body_start(doc: &str, end: usize) -> usize {
    let (line_end, next) = line_at(doc, end);
    if text(doc, end..line_end)
        .trim_matches([' ', '\t'])
        .is_empty()
    {
        next
    } else {
        end
    }
}
