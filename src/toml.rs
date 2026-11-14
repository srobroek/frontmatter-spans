//! TOML front matter, parsed with `toml_edit`.

use std::ops::Range;

use toml_edit::{Document, Item, Key, Table, Value as TomlValue};

use crate::{Entry, Format, LineIndex, Node, ParseError, Value};

struct Cx<'a> {
    base: usize,
    index: &'a LineIndex<'a>,
}

impl Cx<'_> {
    fn span(&self, range: Range<usize>) -> crate::Span {
        self.index
            .span(self.base + range.start..self.base + range.end)
    }
}

pub(crate) fn parse(content: &str, base: usize, index: &LineIndex<'_>) -> Result<Node, ParseError> {
    let cx = Cx { base, index };
    let doc = Document::parse(content).map_err(|e| ParseError {
        format: Format::Toml,
        message: e.message().to_owned(),
        span: e.span().map(|r| cx.span(r)),
    })?;
    let whole = 0..content.len();
    Ok(table(doc.as_table(), &whole, &cx))
}

fn table(t: &Table, fallback: &Range<usize>, cx: &Cx<'_>) -> Node {
    let entries: Vec<Entry> = t
        .iter()
        .filter_map(|(k, item)| {
            let key = t.key(k)?;
            Some(entry(key, item, fallback, cx))
        })
        .collect();
    let range = cover(t.span().as_ref(), &entries, fallback, cx);
    Node {
        value: Value::Table(entries),
        span: cx.span(range),
    }
}

/// The table's own span (its header) widened to cover every entry. A table with no header,
/// such as the root or a dotted-key parent, takes the entries alone.
fn cover(
    own: Option<&Range<usize>>,
    entries: &[Entry],
    fallback: &Range<usize>,
    cx: &Cx<'_>,
) -> Range<usize> {
    let starts = entries.iter().map(|e| e.key_span.range.start);
    let ends = entries.iter().map(|e| e.value.span.range.end);
    let lo = starts.chain(own.iter().map(|r| r.start + cx.base)).min();
    let hi = ends.chain(own.iter().map(|r| r.end + cx.base)).max();
    match (lo, hi) {
        (Some(lo), Some(hi)) => lo - cx.base..hi - cx.base,
        _ => fallback.clone(),
    }
}

fn entry(key: &Key, item: &Item, fallback: &Range<usize>, cx: &Cx<'_>) -> Entry {
    let key_range = key.span().unwrap_or_else(|| fallback.clone());
    Entry {
        key: key.get().to_owned(),
        key_span: cx.span(key_range.clone()),
        value: item_node(item, &key_range, cx),
    }
}

fn item_node(item: &Item, fallback: &Range<usize>, cx: &Cx<'_>) -> Node {
    match item {
        Item::None => Node {
            value: Value::Null,
            span: cx.span(fallback.clone()),
        },
        Item::Value(v) => value_node(v, fallback, cx),
        Item::Table(t) => table(t, fallback, cx),
        Item::ArrayOfTables(a) => {
            let tables: Vec<Node> = a.iter().map(|t| table(t, fallback, cx)).collect();
            let lo = tables.iter().map(|n| n.span.range.start).min();
            let hi = tables.iter().map(|n| n.span.range.end).max();
            let range = match (lo, hi) {
                (Some(lo), Some(hi)) => lo - cx.base..hi - cx.base,
                _ => a.span().unwrap_or_else(|| fallback.clone()),
            };
            Node {
                value: Value::Array(tables),
                span: cx.span(range),
            }
        }
    }
}

fn value_node(v: &TomlValue, fallback: &Range<usize>, cx: &Cx<'_>) -> Node {
    let range = v.span().unwrap_or_else(|| fallback.clone());
    let value = match v {
        TomlValue::String(s) => Value::String(s.value().clone()),
        TomlValue::Integer(i) => Value::Integer(*i.value()),
        TomlValue::Float(f) => Value::Float(*f.value()),
        TomlValue::Boolean(b) => Value::Bool(*b.value()),
        TomlValue::Datetime(d) => Value::Datetime(d.value().to_string()),
        TomlValue::Array(a) => {
            Value::Array(a.iter().map(|item| value_node(item, &range, cx)).collect())
        }
        TomlValue::InlineTable(t) => Value::Table(
            t.iter()
                .filter_map(|(k, item)| {
                    let (key, _) = t.get_key_value(k)?;
                    let key_range = key.span().unwrap_or_else(|| range.clone());
                    Some(Entry {
                        key: key.get().to_owned(),
                        key_span: cx.span(key_range.clone()),
                        value: value_node(item, &key_range, cx),
                    })
                })
                .collect(),
        ),
    };
    Node {
        value,
        span: cx.span(range),
    }
}
