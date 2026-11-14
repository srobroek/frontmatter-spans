//! Shared span invariants: every node and key slice is a clean, re-parsable fragment.

#![allow(dead_code)]

use frontmatter_spans::{Format, FrontMatter, Node, Value};

/// A canonical, span-free rendering of a value, for comparing a node to its re-parse.
pub(crate) fn shape(node: &Node) -> String {
    match &node.value {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => format!("{f:?}"),
        Value::String(s) | Value::Datetime(s) => format!("{s:?}"),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(shape).collect();
            format!("[{}]", inner.join(","))
        }
        Value::Table(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|e| format!("{:?}:{}", e.key, shape(&e.value)))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Parent {
    Root,
    MapValue,
    SeqItem,
}

struct Visited<'a> {
    node: &'a Node,
    parent: Parent,
}

fn walk<'a>(
    node: &'a Node,
    parent: Parent,
    out: &mut Vec<Visited<'a>>,
    keys: &mut Vec<(&'a str, std::ops::Range<usize>)>,
) {
    out.push(Visited { node, parent });
    match &node.value {
        Value::Table(entries) => {
            for e in entries {
                keys.push((&e.key, e.key_span.range.clone()));
                walk(&e.value, Parent::MapValue, out, keys);
            }
        }
        Value::Array(items) => items
            .iter()
            .for_each(|n| walk(n, Parent::SeqItem, out, keys)),
        _ => {}
    }
}

fn parse_fragment(format: Format, fragment: &str) -> Option<FrontMatter<'static>> {
    let doc: &'static str = Box::leak(
        match format {
            Format::Yaml => format!("---\n{fragment}\n---\n"),
            Format::Toml => format!("+++\nv = {fragment}\n+++\n"),
            Format::Json => format!("{{\"v\": {fragment}}}"),
        }
        .into_boxed_str(),
    );
    FrontMatter::parse(doc).ok().flatten()
}

/// Whether the node is a collection, and where its first entry starts (`None` when empty).
fn first_entry(node: &Node) -> (bool, Option<usize>) {
    match &node.value {
        Value::Table(entries) => (true, entries.first().map(|e| e.key_span.range.start)),
        Value::Array(items) => (true, items.first().map(|n| n.span.range.start)),
        _ => (false, None),
    }
}

/// Whether the text before `start` ends with a block scalar header (`|`, `>`, with chomping).
fn after_block_header(doc: &str, start: usize) -> bool {
    let before = doc[..start].trim_end();
    let line = before.rsplit(['\n', '\r']).next().unwrap_or("");
    let line = line.split(" #").next().unwrap_or(line).trim_end();
    ["|", ">", "|-", ">-", "|+", ">+"].iter().any(|h| {
        line.ends_with(h)
            && line[..line.len() - h.len()]
                .chars()
                .last()
                .is_none_or(|c| c == ' ' || c == '-')
    })
}

/// Whether the last line of `text` carries comment text: a `#` outside quotes (quotes may
/// span lines) at the start of the line or after a blank.
fn has_trailing_comment(text: &str) -> bool {
    let b = text.as_bytes();
    let last_line = b
        .iter()
        .rposition(|c| matches!(c, b'\n' | b'\r'))
        .map_or(0, |i| i + 1);
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'\'' => {
                i += 1;
                while i < b.len() {
                    if b[i] == b'\'' {
                        if b.get(i + 1) == Some(&b'\'') {
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    i += 1;
                }
            }
            b'#' if i >= last_line && (i == last_line || b[i - 1] == b' ' || b[i - 1] == b'\t') => {
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Parse `doc` and assert the span invariants for every node and key. Returns the root shape.
#[allow(clippy::too_many_lines)]
pub(crate) fn check(doc: &str) -> String {
    let fm = FrontMatter::parse(doc)
        .unwrap_or_else(|e| panic!("{doc:?}: {e}"))
        .unwrap_or_else(|| panic!("{doc:?}: no block"));
    let format = fm.block.format;
    let mut all = Vec::new();
    let mut keys = Vec::new();
    walk(&fm.root, Parent::Root, &mut all, &mut keys);

    for Visited { node, parent } in all.iter().skip(1) {
        let slice = &doc[node.span.range.clone()];
        let (is_collection, first) = first_entry(node);
        if format == Format::Yaml && is_collection {
            // A YAML collection is zero-width at its first entry, or at its bracket if empty.
            assert!(
                node.span.range.is_empty(),
                "collection {slice:?} of {doc:?}"
            );
            match first {
                Some(at) => assert_eq!(node.span.range.start, at, "{doc:?}"),
                None => assert!(
                    matches!(doc.as_bytes()[node.span.range.start], b'[' | b'{'),
                    "empty collection not at its bracket in {doc:?}"
                ),
            }
            continue;
        }
        if slice.is_empty() {
            if format == Format::Yaml && !after_block_header(doc, node.span.range.start) {
                // An omitted value is zero-width right after its `:` or `-`, or at the end
                // of that line when a comment follows.
                let at = node.span.range.start;
                let want = match parent {
                    Parent::MapValue => b':',
                    Parent::SeqItem => b'-',
                    Parent::Root => unreachable!(),
                };
                let rest = &doc.as_bytes()[at..];
                let eol = rest.first().is_none_or(|b| matches!(b, b'\n' | b'\r'));
                assert!(
                    (at > 0 && doc.as_bytes()[at - 1] == want) || eol,
                    "empty value at {at} is misplaced in {doc:?}"
                );
            }
            continue;
        }
        if format == Format::Yaml && after_block_header(doc, node.span.range.start) {
            // Block scalar content keeps its significant trailing blanks, but not its line end.
            assert!(
                !slice.ends_with(['\n', '\r']),
                "terminator in {slice:?} of {doc:?}"
            );
            assert_eq!(
                slice.trim_start(),
                slice,
                "indentation in {slice:?} of {doc:?}"
            );
            if let Value::String(value) = &node.value {
                // Significant trailing blanks of the last content line are inside the span.
                let body = value.trim_end_matches(['\n', '\r']);
                let blanks = &body[body.trim_end_matches([' ', '\t']).len()..];
                assert!(
                    slice.ends_with(blanks),
                    "lost blanks in {slice:?} of {doc:?}"
                );
            }
            continue; // and it is not a fragment on its own
        }
        assert_eq!(slice, slice.trim(), "padding in {slice:?} of {doc:?}");
        // TOML tables written with headers or dotted keys are not fragments.
        let toml_table_form = format == Format::Toml
            && match &node.value {
                Value::Table(_) => !slice.starts_with('{'),
                Value::Array(items) => {
                    slice.starts_with("[[")
                        && matches!(items.first().map(|n| &n.value), Some(Value::Table(_)))
                }
                _ => false,
            };
        if toml_table_form {
            continue;
        }
        if format == Format::Yaml && after_block_header(doc, node.span.range.start) {
            continue; // block scalar content is not a fragment on its own
        }
        if format == Format::Yaml {
            assert!(
                !has_trailing_comment(slice),
                "comment text in {slice:?} of {doc:?}"
            );
        }
        let fragment = if format == Format::Yaml {
            // Block collections continue at the column where they began.
            format!("{}{slice}", " ".repeat(node.span.start.column - 1))
        } else {
            slice.to_owned()
        };
        let again = parse_fragment(format, &fragment)
            .unwrap_or_else(|| panic!("{slice:?} of {doc:?} does not re-parse"));
        let got = match format {
            Format::Yaml => &again.root,
            _ => again.root.get("v").expect("v"),
        };
        assert_eq!(shape(got), shape(node), "{slice:?} of {doc:?}");
    }

    for (key, range) in keys {
        let slice = &doc[range];
        assert_eq!(slice, slice.trim(), "padding in key {slice:?}");
        // Dotted TOML keys (`a.b`) are paths, not single keys.
        if format == Format::Toml && slice.contains('.') && !slice.starts_with(['"', '\'']) {
            continue;
        }
        let wrapped = match format {
            Format::Yaml => format!("---\n{slice}: 1\n---\n"),
            Format::Toml => format!("+++\n{slice} = 1\n+++\n"),
            Format::Json => format!("{{{slice}: 1}}"),
        };
        let again = FrontMatter::parse(&wrapped)
            .unwrap_or_else(|e| panic!("key {slice:?} of {doc:?}: {e}"))
            .unwrap();
        let Value::Table(entries) = &again.root.value else {
            panic!("not a table");
        };
        assert_eq!(entries[0].key, key, "key {slice:?} of {doc:?}");
    }
    shape(&fm.root)
}
