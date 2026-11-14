//! Regression tests from review round 1 of PR 351.

use frontmatter_spans::{FrontMatter, Node, Value};
use serde::Deserialize;

fn text<'a>(doc: &'a str, node: &Node) -> &'a str {
    &doc[node.span.range.clone()]
}

#[test]
fn duplicate_json_members_are_all_kept_and_lookup_takes_the_last() {
    let doc = r#"{"a": 1, "b": 0, "a": 2}"#;
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let Value::Table(entries) = &fm.root.value else {
        panic!("not a table");
    };
    let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["a", "b", "a"]);
    assert_eq!(entries[0].value.value, Value::Integer(1));

    let last = fm.root.entry("a").unwrap();
    assert_eq!(last.value.value, Value::Integer(2));
    assert_eq!(&doc[last.key_span.range.clone()], "\"a\"");
    assert_eq!(last.key_span.range.start, 17);
    assert_eq!(text(doc, &last.value), "2");
    assert_eq!(text(doc, fm.get("a").unwrap()), "2");
}

#[test]
fn yaml_quoted_scalar_span_stops_at_the_closing_quote_before_a_comment() {
    let cases = [
        ("a: \"x\" # note", "\"x\""),
        ("a: 'it''s' # note", "'it''s'"),
        ("a: \"q\\\"uote\" # note", "\"q\\\"uote\""),
        ("a:\n  \"x\" # note", "\"x\""),
    ];
    for (body, want) in cases {
        let doc = format!("---\n{body}\n---\n");
        let fm = FrontMatter::parse(&doc).unwrap().unwrap();
        assert_eq!(text(&doc, fm.get("a").unwrap()), want, "{body}");
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum Shape {
    Pair(u8, u8),
}

#[test]
fn sequences_longer_than_the_target_are_an_error_at_the_first_extra_element() {
    // Byte offsets of each `3`: line `t` starts at 4, `f` at 17, `v` at 30.
    let doc = "---\nt: [1, 2, 3]\nf: [1, 2, 3]\nv: {Pair: [1, 2, 3]}\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();
    let cases = [
        (
            fm.get("t").unwrap().deserialize::<(u8, u8)>().unwrap_err(),
            14,
        ),
        (
            fm.get("f").unwrap().deserialize::<[u8; 2]>().unwrap_err(),
            27,
        ),
        (fm.get("v").unwrap().deserialize::<Shape>().unwrap_err(), 47),
    ];
    for (err, extra_at) in cases {
        let span = err.span.clone().expect("span");
        assert_eq!(span.range, extra_at..extra_at + 1, "{err}");
        assert_eq!(&doc[span.range], "3");
    }
    assert_eq!(
        fm.get("t").unwrap().deserialize::<Vec<u8>>().unwrap(),
        [1, 2, 3]
    );
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
#[allow(dead_code)]
struct Slug(String);

impl TryFrom<String> for Slug {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        if s.contains(' ') {
            Err(format!("`{s}` is not a slug"))
        } else {
            Ok(Self(s))
        }
    }
}

#[test]
fn validation_errors_after_a_successful_read_get_the_value_span() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Page {
        slug: Slug,
        other: u8,
    }
    let doc = "---\nslug: bad slug\nother: 1\n---\n";
    let fm = FrontMatter::parse(doc).unwrap().unwrap();

    let nested = fm.deserialize::<Page>().unwrap_err();
    assert!(nested.message.contains("not a slug"), "{nested}");
    assert_eq!(&doc[nested.span.unwrap().range], "bad slug");

    let single = fm.deserialize_key::<Slug>("slug").unwrap_err();
    assert_eq!(&doc[single.span.unwrap().range], "bad slug");
}
