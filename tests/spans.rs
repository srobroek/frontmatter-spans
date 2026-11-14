//! Span invariants over a corpus, plus named regressions.
//!
//! YAML collection spans are zero-width at the start of their first entry. TOML and JSON
//! collection spans come straight from `toml_edit` and the crate's own JSON parser.

mod common;

use std::time::{Duration, Instant};

use common::check;
use frontmatter_spans::{FrontMatter, Node, Value};

const CORPUS: &[&str] = &[
    // YAML: nested flow, immediately nested, with trailing comments.
    "---\na: [[1], [2]]\nb: {c: {d: 1}, e: [x, y]}\n---\n",
    "---\na: [[1],[2]] # note\nb: {c: {d: 1}} # note\n---\n",
    "---\na: [1, # inner\n  2]\nb: 3\n---\n",
    // YAML: trailing commas, empty values, block collections with comments and blank lines.
    "---\na: {b: 1,}\nc: [1, 2,]\nd:\ne: []\nf: {}\n---\n",
    "---\na:\n  - 1\n  - 2 # two\n\n  # tail\nb:\n  c: 1 # one\n  d: 2   # two\n\n# end\ne: 3\n---\n",
    "---\nlist:\n  - name: a\n    tags: [x, y]\n  - name: b # second\n    tags: []\n  -\n---\n",
    "---\n- a\n- b # c\n- [d, e]\n- {f: g}\n---\n",
    // YAML: quotes, block scalars, multi-line scalars, multi-byte text.
    "---\nq: \"x # not a comment\" # comment\ns: 'it''s' # c\nu: ünï-ćödé\nv:\nw: [\"é\", 'ü']\n---\n",
    "---\nl: |\n  one\n  two\nf: >-\n  a\n  b\nm: \"folds\n  over\"\np: plain\n  folds\nk y: 1\n'q''s': 2\n---\n",
    // TOML.
    "+++\ntitle = \"x\" # comment\nlist = [1, 2, [3]] # note\ninline = {a = {b = 1}, c = [1]}\n[sec]\nk = 1\nm = \"ü\"\n[[t]]\nq = 1\n[[t]]\nq = 2\n+++\n",
    "+++\na.b = 1\n'quoted key' = 2\nd = 2024-05-06T07:08:09Z\n+++\n",
    // JSON.
    "{\"a\": [1, [2, 3], {\"b\": null}], \"c\": {\"d\": \"é\"}, \"e\": 1.5e3}\n",
    "{\n  \"a\": [\n    1,\n    {\"b\": [true, false]}\n  ],\n  \"c\": \"ü\"\n}\nbody",
];

#[test]
fn every_span_is_a_clean_reparsable_fragment() {
    for doc in CORPUS {
        check(doc);
        check(&doc.replace('\n', "\r\n"));
        check(&format!("\u{feff}{doc}"));
    }
}

// ---- named regressions ----------------------------------------------------------------

fn text<'a>(doc: &'a str, node: &Node) -> &'a str {
    &doc[node.span.range.clone()]
}

#[test]
fn yaml_collection_spans_are_zero_width_at_the_first_entry() {
    // (body, key, byte offset of the first entry within the document)
    let doc =
        "---\na: [[1],[2]]\nb: {c: {d: 1}}\nc:\n  - x\n  - y\nd:\n  e: 1\nf: []\ng: {}\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let at = |key: &str| {
        let n = fm.get(key).unwrap();
        assert!(n.span.range.is_empty(), "{key}: {:?}", n.span.range);
        n.span.range.start
    };
    // The first entry of `a` is `[1]`, which is itself zero-width at its first entry.
    assert_eq!(&doc[at("a")..][..1], "1");
    assert_eq!(&doc[at("b")..][..1], "c");
    assert_eq!(&doc[at("c")..][..1], "x");
    assert_eq!(&doc[at("d")..][..1], "e");
    // An empty flow collection has no entry, so it sits at its opening bracket.
    assert_eq!(&doc[at("f")..][..1], "[");
    assert_eq!(&doc[at("g")..][..1], "{");
    // The root is a block mapping: zero-width at its first key.
    assert!(fm.root.span.range.is_empty());
    assert_eq!(&doc[fm.root.span.range.start..][..1], "a");
}

#[test]
fn a_nested_collection_after_a_multiline_quoted_string_does_not_borrow_sibling_text() {
    let doc = "---\na: [\"x-\n # n\", [1]]\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let Value::Array(items) = &fm.get("a").unwrap().value else {
        panic!("not an array");
    };
    let inner = &items[1];
    assert!(inner.span.range.is_empty());
    assert_eq!(&doc[inner.span.range.start..][..1], "1");
    assert_eq!(text(doc, &items[0]), "\"x-\n # n\"");
}

#[test]
fn an_empty_value_is_null_and_zero_width() {
    for (doc, key) in [
        ("---\na:\nb: 1\n---\n", "a"),
        ("---\na: # note\nb: 1\n---\n", "a"),
        ("---\nx:\n  draft:\n---\n", "draft"),
    ] {
        let fm = FrontMatter::parse(doc).unwrap().unwrap();
        let node = fm
            .get(key)
            .or_else(|| fm.get("x").and_then(|x| x.get(key)))
            .unwrap();
        assert_eq!(node.value, Value::Null, "{doc:?}");
        assert!(node.span.range.is_empty(), "{doc:?}");
    }
    // Without a comment it sits right after the colon.
    let doc = "---\na:\nb: 1\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(fm.get("a").unwrap().span.range, 6..6);
    // And an omitted sequence entry sits right after its dash.
    let doc = "---\n- a\n-\n- b\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let Value::Array(items) = &fm.root.value else {
        panic!()
    };
    assert_eq!(items[1].span.range, 9..9);
}

#[test]
fn empty_block_scalars_are_empty_strings_not_omitted_values() {
    let cases = [
        ("---\na: |-\n---\n", "|-"),
        ("---\na: >-\n---\n", ">-"),
        ("---\na: |\nb: 1\n---\n", "|"),
        ("---\na: >\nb: 1\n---\n", ">"),
        ("---\na: |+\nb: 1\n---\n", "|+"),
        ("---\na: >+\nb: 1\n---\n", ">+"),
    ];
    for (doc, header) in cases {
        let fm = FrontMatter::parse(doc).unwrap().unwrap();
        let a = fm.get("a").unwrap();
        assert_eq!(a.value, Value::String(String::new()), "{doc:?}");
        // Not the omitted-value position (right after the colon): the span is at or after the header.
        let header_at = doc.find(header).unwrap();
        assert!(
            a.span.range.start >= header_at,
            "{doc:?}: {:?}",
            a.span.range
        );
    }
}

#[test]
fn a_bare_alias_error_covers_the_whole_alias_token() {
    let cases = [
        ("b: *x", "*x"),
        ("b: *long-name # c", "*long-name"),
        ("b: [*x, 2]", "*x"),
        ("b: {c: *é1}", "*é1"),
        ("- *list_1\n- 2", "*list_1"),
    ];
    for (body, token) in cases {
        let doc = format!("---\n{body}\n---\n");
        let err = FrontMatter::parse(&doc).unwrap_err();
        let span = err.span.clone().expect("span");
        assert_eq!(&doc[span.range], token, "{body:?}: {err}");
    }
}

#[test]
fn a_very_wide_single_line_flow_array_parses_in_linear_time() {
    // 250,000 singleton arrays is about 1 MiB on one line.
    let doc = format!("---\na: [{}[1]]\n---\n", "[1],".repeat(250_000));
    let start = Instant::now();
    let fm = FrontMatter::parse(&doc).unwrap().unwrap();
    let Value::Array(items) = &fm.get("a").unwrap().value else {
        panic!("not an array");
    };
    assert_eq!(items.len(), 250_001);
    assert!(
        start.elapsed() < Duration::from_secs(20),
        "took {:?}",
        start.elapsed()
    );
}

#[test]
fn block_scalar_spans_keep_significant_trailing_whitespace() {
    // `(body, key, exact slice)`: only the line terminator, trailing blank lines and the
    // parser-consumed indentation are left out.
    let cases = [
        ("a: |-\n  hi  \nb: 1", "hi  "),
        ("a: |\n  hi  \nb: 1", "hi  "),
        ("a: |+\n  hi  \n\nb: 1", "hi  "),
        ("a: >-\n  hi\t\nb: 1", "hi\t"),
        ("a: >\n  hi \t\nb: 1", "hi \t"),
        ("a: >+\n  hi  \n\nb: 1", "hi  "),
        ("a: >-\n  one  \n  two \nb: 1", "one  \n  two "),
        ("a: |\n  hi\n  \nb: 1", "hi"),
        ("a: |-\n  hi  \n", "hi  "),
        ("a: |\r\n  hi  \r\nb: 1", "hi  "),
    ];
    for (body, want) in cases {
        let doc = format!("---\n{body}\n---\n");
        let fm = FrontMatter::parse(&doc).unwrap().unwrap();
        let a = fm.get("a").unwrap();
        assert_eq!(text(&doc, a), want, "{body:?}");
    }
}
