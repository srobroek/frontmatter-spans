//! Behaviour of `frontmatter-spans` through its public API.

use frontmatter_spans::{Format, FrontMatter, LineCol, LineIndex, Node, Value, detect};
use serde::Deserialize;

fn text<'a>(doc: &'a str, node: &Node) -> &'a str {
    &doc[node.span.range.clone()]
}

fn lc(line: usize, column: usize) -> LineCol {
    LineCol { line, column }
}

// ---- detection ------------------------------------------------------------------------

/// `(document, format, content, body)`; each case worked out by reading the bytes.
const DETECTED: &[(&str, Format, &str, &str)] = &[
    ("---\na: 1\n---\nbody", Format::Yaml, "a: 1\n", "body"),
    ("---\na: 1\n...\nbody", Format::Yaml, "a: 1\n", "body"),
    (
        "---\r\na: 1\r\n---\r\nbody",
        Format::Yaml,
        "a: 1\r\n",
        "body",
    ),
    ("---\ra: 1\r---\rbody", Format::Yaml, "a: 1\r", "body"),
    (
        "\u{feff}---\na: 1\n---\nbody",
        Format::Yaml,
        "a: 1\n",
        "body",
    ),
    ("---  \na: 1\n---\t\nbody", Format::Yaml, "a: 1\n", "body"),
    ("---\n---\nbody", Format::Yaml, "", "body"),
    ("---\na: 1\n---", Format::Yaml, "a: 1\n", ""),
    ("+++\na = 1\n+++\nbody", Format::Toml, "a = 1\n", "body"),
    (
        "+++\r\na = 1\r\n+++\r\nbody",
        Format::Toml,
        "a = 1\r\n",
        "body",
    ),
    ("{\"a\": 1}\nbody", Format::Json, "{\"a\": 1}", "body"),
    (
        "{\"a\": \"}\"}\nbody",
        Format::Json,
        "{\"a\": \"}\"}",
        "body",
    ),
    (
        "{\"a\": {\"b\": 1}}\r\nbody",
        Format::Json,
        "{\"a\": {\"b\": 1}}",
        "body",
    ),
    ("{\"a\": 1} tail", Format::Json, "{\"a\": 1}", " tail"),
    ("\u{feff}{\"a\": 1}", Format::Json, "{\"a\": 1}", ""),
];

#[test]
fn detects_each_format() {
    for &(doc, format, content, body) in DETECTED {
        let block = detect(doc).unwrap_or_else(|| panic!("no block in {doc:?}"));
        assert_eq!(block.format, format, "{doc:?}");
        assert_eq!(&doc[block.content.clone()], content, "{doc:?}");
        assert_eq!(&doc[block.body_start..], body, "{doc:?}");
    }
}

#[test]
fn outer_range_covers_both_delimiters_without_the_terminator() {
    let doc = "\u{feff}---\r\na: 1\r\n---\r\nbody";
    let block = detect(doc).unwrap();
    assert_eq!(&doc[block.outer], "---\r\na: 1\r\n---");
}

const NOT_FRONT_MATTER: &[&str] = &[
    "",
    "plain text\n",
    "---\na: 1\n",        // never closes
    "+++\na = 1\n",       // never closes
    "---",                // opening line has no terminator
    "{\"a\": 1",          // unclosed object
    "{\"a\": \"}\"",      // the only `}` is inside a string
    "\n---\na: 1\n---\n", // not at the start
    " ---\na: 1\n---\n",  // indented fence
    "----\na: 1\n----\n", // longer fence
    "--- x\na: 1\n---\n", // text after the opening fence
    "---\na: 1\n+++\n",   // closing fence of the wrong kind
    "+++\na = 1\n...\n",  // `...` closes YAML only
];

#[test]
fn rejects_documents_without_a_closed_block_at_the_start() {
    for doc in NOT_FRONT_MATTER {
        assert_eq!(detect(doc), None, "{doc:?}");
        assert!(FrontMatter::parse(doc).unwrap().is_none(), "{doc:?}");
    }
}

// ---- spans ----------------------------------------------------------------------------

#[test]
fn yaml_spans_index_the_whole_document_through_bom_and_crlf() {
    let doc = "\u{feff}---\r\nname: Ada\r\nnote: ünï\r\n---\r\nbody";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();

    let name = fm.root.entry("name").unwrap();
    assert_eq!(&doc[name.key_span.range.clone()], "name");
    assert_eq!(name.key_span.start, lc(2, 1));
    assert_eq!(text(doc, &name.value), "Ada");
    assert_eq!(name.value.span.start, lc(2, 7));
    assert_eq!(name.value.span.end, lc(2, 10));

    let note = fm.get("note").unwrap();
    assert_eq!(note.value, Value::String("ünï".into()));
    assert_eq!(text(doc, note), "ünï");
    assert_eq!(note.span.range, 25..30); // 3 BOM + 5 opener + 11 name line + 6 `note: `, then 5 bytes
    assert_eq!(note.span.start, lc(3, 7));
    assert_eq!(note.span.end, lc(3, 10)); // three code points, six bytes
}

#[test]
fn yaml_block_scalar_span_drops_the_trailing_newline() {
    let doc = "---\ntext: |\n  hi\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(text(doc, fm.get("text").unwrap()), "hi");
}

#[test]
fn yaml_quoted_scalar_spans_include_the_quotes() {
    let doc = "---\na: 'x'\nb: \"y\"\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(text(doc, fm.get("a").unwrap()), "'x'");
    assert_eq!(text(doc, fm.get("b").unwrap()), "\"y\"");
}

#[test]
fn yaml_scalar_types_and_non_string_keys() {
    let doc = "---\na: null\nb: true\nc: 3\nd: 1.5\n4: four\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(fm.get("a").unwrap().value, Value::Null);
    assert_eq!(fm.get("b").unwrap().value, Value::Bool(true));
    assert_eq!(fm.get("c").unwrap().value, Value::Integer(3));
    assert_eq!(fm.get("d").unwrap().value, Value::Float(1.5));
    assert_eq!(fm.get("4").unwrap().value, Value::String("four".into()));
}

#[test]
fn empty_yaml_block_is_null_with_an_empty_span_in_the_document() {
    let doc = "---\n---\nbody";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(fm.root.value, Value::Null);
    assert_eq!(fm.root.span.range, 4..4);
    assert_eq!(fm.get("a"), None);
}

#[test]
fn toml_spans_index_the_whole_document() {
    let doc = "\u{feff}+++\r\ntitle = \"Hi\"\r\n[sec]\r\nk = [1, {z = 2}]\r\n+++\r\nbody";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();

    let title = fm.root.entry("title").unwrap();
    assert_eq!(&doc[title.key_span.range.clone()], "title");
    assert_eq!(text(doc, &title.value), "\"Hi\"");
    assert_eq!(title.value.span.start, lc(2, 9));

    let sec = fm.get("sec").unwrap();
    let k = sec.get("k").unwrap();
    assert_eq!(text(doc, k), "[1, {z = 2}]");
    assert_eq!(k.span.start, lc(4, 5));
    let Value::Array(items) = &k.value else {
        panic!("not an array");
    };
    assert_eq!(text(doc, &items[1]), "{z = 2}");
    assert_eq!(text(doc, items[1].get("z").unwrap()), "2");
    // The table covers its header through its last value.
    assert_eq!(text(doc, sec), "[sec]\r\nk = [1, {z = 2}]");
}

#[test]
fn toml_dotted_keys_and_arrays_of_tables() {
    let doc = "+++\na.b = 1\n[[t]]\nq = 1\n[[t]]\nq = 2\n+++\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(text(doc, fm.get("a").unwrap().get("b").unwrap()), "1");
    assert_eq!(text(doc, fm.get("a").unwrap()), "a.b = 1");
    let Value::Array(tables) = &fm.get("t").unwrap().value else {
        panic!("not an array");
    };
    assert_eq!(tables.len(), 2);
    assert_eq!(text(doc, tables[1].get("q").unwrap()), "2");
}

#[test]
fn toml_datetime_keeps_its_text_form() {
    let doc = "+++\nd = 2024-05-06T07:08:09Z\n+++\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(
        fm.get("d").unwrap().value,
        Value::Datetime("2024-05-06T07:08:09Z".into())
    );
}

#[test]
fn json_spans_index_the_whole_document_and_keys_include_quotes() {
    let doc = "\u{feff}{\n  \"a\": [1, \"é\", null],\n  \"b\": {\"c\": 1.5e3}\n}\nbody";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let a = fm.root.entry("a").unwrap();
    assert_eq!(&doc[a.key_span.range.clone()], "\"a\"");
    assert_eq!(a.key_span.start, lc(2, 3));
    let Value::Array(items) = &a.value.value else {
        panic!("not an array");
    };
    assert_eq!(text(doc, &items[1]), "\"é\"");
    assert_eq!(items[1].span.start, lc(2, 12));
    assert_eq!(items[1].span.end, lc(2, 15));
    assert_eq!(items[2].value, Value::Null);
    let c = fm.get("b").unwrap().get("c").unwrap();
    assert_eq!(c.value, Value::Float(1500.0));
    assert_eq!(text(doc, c), "1.5e3");
    assert_eq!(fm.block.body_start, doc.len() - "body".len());
}

#[test]
fn json_values_decode_escapes_and_integer_range() {
    let doc = r#"{"s": "a\n\u00e9\ud83d\ude00\/", "big": 9223372036854775808, "n": -0, "i": -12}"#;
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(fm.get("s").unwrap().value, Value::String("a\né😀/".into()));
    assert_eq!(
        fm.get("big").unwrap().value,
        Value::Float(9_223_372_036_854_775_808.0)
    );
    assert_eq!(fm.get("n").unwrap().value, Value::Integer(0));
    assert_eq!(fm.get("i").unwrap().value, Value::Integer(-12));
}

// ---- errors ---------------------------------------------------------------------------

#[test]
fn parse_errors_locate_the_failure_in_the_whole_document() {
    // (document, line, column) of the failure; found by reading the offending byte.
    let cases = [
        ("---\nok: 1\nbad: [1\n---\n", Format::Yaml, 4),
        ("+++\nok = 1\nbad = \n+++\n", Format::Toml, 3),
        ("{\n  \"a\": 1,\n  \"b\": ]\n}", Format::Json, 3),
    ];
    for (doc, format, line) in cases {
        let err = FrontMatter::parse(doc).unwrap_err();
        assert_eq!(err.format, format, "{doc:?}");
        let span = err
            .span
            .clone()
            .unwrap_or_else(|| panic!("no span for {doc:?}"));
        assert_eq!(span.start.line, line, "{doc:?}: {err}");
    }
}

#[test]
fn json_error_points_at_the_offending_byte() {
    let doc = "{\"a\": 1,}\n";
    let err = FrontMatter::parse(doc).unwrap_err();
    assert_eq!(err.span.unwrap().range, 8..8);
}

const BAD_JSON: &[&str] = &[
    "{\"a\": 1,}",
    "{a: 1}",
    "{'a': 1}",
    "{\"a\": 01}",
    "{\"a\": 1.}",
    "{\"a\": \"\\x\"}",
    "{\"a\": \"\\ud800\"}",
    "{\"a\": \"tab\there\"}",
    "{\"a\": NaN}",
    "{\"a\": 1 // c\n}",
];

#[test]
fn json_is_strict() {
    for doc in BAD_JSON {
        assert!(
            FrontMatter::parse(doc).is_err(),
            "{doc:?} should be rejected"
        );
    }
}

#[test]
fn deeply_nested_json_is_an_error_not_a_crash() {
    let deep = format!("{{\"a\":{}1{}}}", "[".repeat(5_000), "]".repeat(5_000));
    let err = FrontMatter::parse(&deep).unwrap_err();
    assert!(err.message.contains("too deep"), "{err}");
}

#[test]
fn yaml_block_with_a_second_document_is_rejected() {
    let err = FrontMatter::parse("---\na: 1\n--- b\n---\n").unwrap_err();
    assert!(err.message.contains("more than one"), "{err}");
}

// ---- serde ----------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct Meta {
    title: String,
    count: u32,
    tags: Vec<String>,
    draft: Option<bool>,
}

const SAME_DATA: &[&str] = &[
    "---\ntitle: T\ncount: 3\ntags: [a, b]\n---\n",
    "+++\ntitle = \"T\"\ncount = 3\ntags = [\"a\", \"b\"]\n+++\n",
    "{\"title\": \"T\", \"count\": 3, \"tags\": [\"a\", \"b\"]}\n",
];

#[test]
fn deserialises_the_whole_block_the_same_in_every_format() {
    for doc in SAME_DATA {
        let fm = FrontMatter::parse(doc).unwrap().unwrap();
        let meta: Meta = fm.deserialize().unwrap();
        assert_eq!(
            meta,
            Meta {
                title: "T".into(),
                count: 3,
                tags: vec!["a".into(), "b".into()],
                draft: None
            },
            "{doc:?}"
        );
    }
}

#[test]
fn type_mismatch_error_carries_the_span_of_the_value() {
    let doc = "---\ntitle: T\ncount: many\ntags: []\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let err = fm.deserialize::<Meta>().unwrap_err();
    let span = err.span.unwrap();
    assert_eq!(&doc[span.range], "many");
    assert_eq!(span.start, lc(3, 8));
}

#[test]
fn unknown_field_error_carries_the_span_of_the_key() {
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Strict {
        a: u8,
    }
    let doc = "+++\na = 1\nextra = 2\n+++\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let err = fm.deserialize::<Strict>().unwrap_err();
    assert_eq!(&doc[err.span.unwrap().range], "extra");
}

#[test]
fn missing_field_error_points_at_the_containing_table() {
    let doc = "---\nouter:\n  title: T\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let err = fm.deserialize_key::<Meta>("outer").unwrap_err();
    assert!(err.message.contains("count"), "{err}");
    // A YAML collection span is zero-width at its first entry.
    let span = err.span.unwrap();
    assert!(span.range.is_empty());
    assert!(doc[span.range.start..].starts_with("title: T"));
}

#[derive(Deserialize, PartialEq, Debug)]
struct Slopvac {
    genre: String,
}

#[test]
fn deserialises_one_key_with_the_span_of_its_value() {
    let doc = "---\nother: 1\nslopvac:\n  genre: docs\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let got = fm.deserialize_key::<Slopvac>("slopvac").unwrap().unwrap();
    assert_eq!(
        got.value,
        Slopvac {
            genre: "docs".into()
        }
    );
    assert!(got.span.range.is_empty());
    assert!(doc[got.span.range.start..].starts_with("genre: docs"));
    assert!(fm.deserialize_key::<Slopvac>("absent").unwrap().is_none());
}

#[test]
fn deserialises_enums_from_strings_and_single_key_tables() {
    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "lowercase")]
    enum Mode {
        Off,
        Level(u8),
    }
    #[derive(Debug, Deserialize, PartialEq)]
    struct Cfg {
        a: Mode,
        b: Mode,
    }
    let doc = "---\na: off\nb: {level: 4}\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    assert_eq!(
        fm.deserialize::<Cfg>().unwrap(),
        Cfg {
            a: Mode::Off,
            b: Mode::Level(4)
        }
    );
}

// ---- line index -----------------------------------------------------------------------

#[test]
fn line_index_follows_the_position_contract() {
    // "\u{feff}a\r\nb\rc\nd😀e": BOM 0..3, a=3, CR LF 4..6, b=6, CR=7, c=8, LF=9, d=10, 😀 11..15, e=15
    let text = "\u{feff}a\r\nb\rc\nd😀e";
    let idx = LineIndex::new(text);
    let cases = [
        (0, lc(1, 1)), // inside the BOM counts as the first column
        (3, lc(1, 1)),
        (4, lc(1, 2)),
        (5, lc(1, 3)), // between CR and LF stays on line 1
        (6, lc(2, 1)),
        (8, lc(3, 1)),
        (10, lc(4, 1)),
        (15, lc(4, 3)),
        (16, lc(4, 4)),
        (99, lc(4, 4)), // clamps to the end
    ];
    for (offset, want) in cases {
        assert_eq!(idx.line_col(offset), want, "offset {offset}");
    }
}

#[test]
fn offset_inside_a_character_moves_back_to_its_start() {
    let idx = LineIndex::new("é!");
    assert_eq!(idx.line_col(1), lc(1, 1));
}
