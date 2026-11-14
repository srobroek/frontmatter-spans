//! The YAML subset: no anchors, aliases or tags; plain keys; core-schema scalars.

use frontmatter_spans::{FrontMatter, Value};
use serde::Deserialize as DeriveDeserialize;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};

fn yaml(body: &str) -> String {
    format!("---\n{body}\n---\n")
}

// ---- anchors, aliases and tags are rejected, with the span of the token ---------------

/// `(body, token, line of the token, word the message must contain)`; lines count the `---`.
const REJECTED: &[(&str, &str, usize, &str)] = &[
    ("a: &x 1", "&x", 2, "anchor"),
    ("&x a: 1", "&x", 2, "anchor"),
    ("a: [&x 1, 2]", "&x", 2, "anchor"),
    ("a: {b: &x 1}", "&x", 2, "anchor"),
    ("a: &x [1, 2]", "&x", 2, "anchor"),
    ("- &x 1", "&x", 2, "anchor"),
    ("a: &x\n  b: 1", "&x", 2, "anchor"),
    ("a: &x\n  - 1", "&x", 2, "anchor"),
    ("a:\n  &x\n  b: 1", "&x", 3, "anchor"),
    ("a: &long-name \"q\" # c", "&long-name", 2, "anchor"),
    ("a: !!str 1", "!!str", 2, "tag"),
    ("a: !custom 1", "!custom", 2, "tag"),
    (
        "a: !<tag:yaml.org,2002:str> 1",
        "!<tag:yaml.org,2002:str>",
        2,
        "tag",
    ),
    ("!!str a: 1", "!!str", 2, "tag"),
    ("a: [!!int 1]", "!!int", 2, "tag"),
    ("a: !!seq\n  - 1", "!!seq", 2, "tag"),
    ("a: !!map\n  b: 1", "!!map", 2, "tag"),
    ("a:\n  !!str\n  x", "!!str", 3, "tag"),
    ("a: !!set\n  ? b", "!!set", 2, "tag"),
    ("a: &x !!str 1", "&x", 2, "anchor"),
];

#[test]
fn anchors_and_tags_are_rejected_at_the_token() {
    for &(body, token, line, word) in REJECTED {
        let doc = yaml(body);
        let err = FrontMatter::parse(&doc).expect_err(body);
        let span = err
            .span
            .clone()
            .unwrap_or_else(|| panic!("no span for {body:?}"));
        assert_eq!(&doc[span.range.clone()], token, "{body:?}: {err}");
        assert_eq!(span.start.line, line, "{body:?}");
        assert!(err.message.contains(word), "{body:?}: {err}");
    }
}

#[test]
fn an_alias_is_rejected_at_the_alias() {
    // The anchor comes first, so use a bare alias: the parser or this crate must refuse it.
    let doc = yaml("b: *x");
    let err = FrontMatter::parse(&doc).unwrap_err();
    let span = err.span.expect("span");
    assert_eq!(&doc[span.range], "*x");
}

#[test]
fn the_message_names_the_policy() {
    let err = FrontMatter::parse(&yaml("a: &x 1")).unwrap_err();
    assert!(err.message.contains("not supported"), "{err}");
}

const ACCEPTED: &[&str] = &[
    "a: \"&x\"",
    "a: '!tag'",
    "a: x&y",
    "a: hi!",
    "a: |\n  &x not an anchor\n  !tag not a tag",
    "a: # &x in a comment\n  1",
];

#[test]
fn ampersands_and_bangs_that_are_not_properties_are_data() {
    for body in ACCEPTED {
        assert!(FrontMatter::parse(&yaml(body)).is_ok(), "{body:?}");
    }
}

#[test]
fn merge_key_is_a_plain_key() {
    let doc = yaml("base: {x: 1}\n<<: {y: 2}");
    let fm = FrontMatter::parse(&doc).unwrap().unwrap();
    let Value::Table(entries) = &fm.root.value else {
        panic!("not a table");
    };
    let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["base", "<<"]);
    assert_eq!(
        fm.get("<<").unwrap().get("y").unwrap().value,
        Value::Integer(2)
    );
}

// ---- keys are strings exactly as written ----------------------------------------------

#[test]
fn keys_are_always_strings_as_written() {
    let doc = yaml("0x10: a\n1.0: b\ntrue: c\nnull: d\n~: e\n010: f\n\"q\": g");
    let fm = FrontMatter::parse(&doc).unwrap().unwrap();
    let Value::Table(entries) = &fm.root.value else {
        panic!("not a table");
    };
    let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["0x10", "1.0", "true", "null", "~", "010", "q"]);
}

// ---- scalar resolution (YAML 1.2 core schema) -----------------------------------------

#[derive(Debug)]
enum Want {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Nan,
    Str(&'static str),
}

use Want::{Bool, Float, Int, Nan, Null, Str};

const SCALARS: &[(&str, Want)] = &[
    ("", Null),
    ("~", Null),
    ("null", Null),
    ("Null", Null),
    ("NULL", Null),
    ("nUll", Str("nUll")),
    ("true", Bool(true)),
    ("True", Bool(true)),
    ("TRUE", Bool(true)),
    ("false", Bool(false)),
    ("False", Bool(false)),
    ("FALSE", Bool(false)),
    ("yes", Str("yes")),
    ("on", Str("on")),
    ("TrUe", Str("TrUe")),
    ("0", Int(0)),
    ("12", Int(12)),
    ("-12", Int(-12)),
    ("+12", Int(12)),
    ("017", Int(17)),
    ("0o17", Int(15)),
    ("0x1F", Int(31)),
    ("0xff", Int(255)),
    ("0o8", Str("0o8")),
    ("0x", Str("0x")),
    ("0xG1", Str("0xG1")),
    ("1_000", Str("1_000")),
    ("1.5", Float(1.5)),
    ("-1.5", Float(-1.5)),
    ("+.5", Float(0.5)),
    (".5", Float(0.5)),
    ("5.", Float(5.0)),
    ("1e3", Float(1000.0)),
    ("1.5E-3", Float(0.0015)),
    (".inf", Float(f64::INFINITY)),
    (".Inf", Float(f64::INFINITY)),
    (".INF", Float(f64::INFINITY)),
    ("+.inf", Float(f64::INFINITY)),
    ("-.inf", Float(f64::NEG_INFINITY)),
    (".nan", Nan),
    (".NaN", Nan),
    (".NAN", Nan),
    ("inf", Str("inf")),
    ("nan", Str("nan")),
    ("1e", Str("1e")),
    ("e3", Str("e3")),
    ("1.2.3", Str("1.2.3")),
    (".", Str(".")),
    ("9223372036854775807", Int(i64::MAX)),
    ("9223372036854775808", Float(9_223_372_036_854_775_808.0)),
    ("0xFFFFFFFFFFFFFFFFFF", Str("0xFFFFFFFFFFFFFFFFFF")),
    ("'1'", Str("1")),
    ("\"null\"", Str("null")),
    ("'true'", Str("true")),
];

#[test]
fn plain_scalars_resolve_by_the_yaml_core_schema() {
    for (text, want) in SCALARS {
        let doc = yaml(&format!("a: {text}"));
        let fm = FrontMatter::parse(&doc).unwrap().unwrap();
        let got = &fm.get("a").unwrap().value;
        let ok = match (want, got) {
            (Null, Value::Null) => true,
            (Bool(w), Value::Bool(g)) => w == g,
            (Int(w), Value::Integer(g)) => w == g,
            (Float(w), Value::Float(g)) => w.to_bits() == g.to_bits(),
            (Nan, Value::Float(g)) => g.is_nan(),
            (Str(w), Value::String(g)) => w == g,
            _ => false,
        };
        assert!(ok, "`{text}` should be {want:?}, got {got:?}");
    }
}

// ---- deserialisation leftovers --------------------------------------------------------

/// Reads the first entry of a table and ignores the rest, as a careless visitor would.
#[derive(Debug)]
#[allow(dead_code)]
struct FirstOnly(String);

impl<'de> Deserialize<'de> for FirstOnly {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = FirstOnly;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a table")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<FirstOnly, A::Error> {
                let first = map
                    .next_key::<String>()?
                    .ok_or_else(|| de::Error::custom("empty"))?;
                let _: de::IgnoredAny = map.next_value()?;
                Ok(FirstOnly(first))
            }
        }
        d.deserialize_map(V)
    }
}

#[test]
fn table_entries_a_visitor_leaves_unread_are_an_error_at_the_first_unread_key() {
    let doc = yaml("one: 1\ntwo: 2\nthree: 3");
    let fm = FrontMatter::parse(&doc).unwrap().unwrap();
    let err = fm.deserialize::<FirstOnly>().unwrap_err();
    let span = err.span.clone().expect("span");
    assert_eq!(&doc[span.range], "two");
    assert!(err.message.contains('3'), "{err}");
}

#[derive(Debug, DeriveDeserialize)]
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
fn a_sequence_element_that_fails_validation_gets_its_own_span() {
    let doc = yaml("tags: [good, bad one, fine]");
    let fm = FrontMatter::parse(&doc).unwrap().unwrap();
    let err = fm
        .get("tags")
        .unwrap()
        .deserialize::<Vec<Slug>>()
        .unwrap_err();
    assert_eq!(&doc[err.span.expect("span").range], "bad one");
}
