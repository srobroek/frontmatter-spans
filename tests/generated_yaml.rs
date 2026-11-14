//! Generated YAML front matter: every span invariant holds for every node of random structures.
//!
//! The generator covers block and flow mappings and sequences nested to depth 4 with up to
//! six entries, flow with and without trailing commas, single-line and multi-line flow, empty
//! values (`k:` and `-`), plain, single-quoted and double-quoted scalars with multi-byte text
//! and escapes, scalars folded over several lines, literal and folded block scalars with strip
//! and clip chomping (also empty ones), keys with spaces and quotes, trailing and leading
//! comments, blank lines, compact and indented sequences, CRLF, and a BOM.

mod common;

use common::check;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Style {
    Plain,
    Single,
    Double,
}

#[derive(Clone, Debug)]
enum N {
    Empty,
    Int(i64),
    Str(String, Style),
    /// A block scalar: its lines, whether folded (`>`), and whether stripped (`-`).
    Block(Vec<String>, bool, bool),
    Seq(Coll<N>),
    Map(Coll<(String, Style, N)>),
}

/// A collection and the rendering choices made for it.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
struct Coll<T> {
    /// Each item with decoration bits: 1 trailing comment, 2 blank line before,
    /// 4 comment line before, 8 no space after a flow colon, 16 fold a scalar over two lines.
    items: Vec<(u8, T)>,
    flow: bool,
    comma: bool,
    multiline: bool,
    /// For a block sequence under a mapping key: dashes at the key's own indent.
    compact: bool,
}

fn scalar() -> impl Strategy<Value = N> {
    prop_oneof![
        Just(N::Empty),
        (-999i64..999).prop_map(N::Int),
        "w[a-zé日]{0,5}".prop_map(|s| N::Str(s, Style::Plain)),
        "w[a-z]{1,3} [a-z日]{1,3}".prop_map(|s| N::Str(s, Style::Plain)),
        "[a-zé日 #:,'\\[\\]{}]{0,6}".prop_map(|s| N::Str(s, Style::Single)),
        "[a-zé日 #:,'\\[\\]{}\"\\\\\n\t]{0,6}".prop_map(|s| N::Str(s, Style::Double)),
        (
            prop::collection::vec("[a-zé日]{1,5}[ \t]{0,2}", 0..3),
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(|(lines, folded, strip)| N::Block(lines, folded, strip)),
    ]
}

/// A key and its style. Plain keys may hold spaces and quotes inside; quoted keys anything.
fn key() -> impl Strategy<Value = (String, Style)> {
    prop_oneof![
        "[a-zé日]([a-zé日 '\"]{0,3}[a-zé日])?".prop_map(|k| (k, Style::Plain)),
        "[a-zé日 '\"]{1,4}".prop_map(|k| (k, Style::Single)),
        "[a-zé日 '\"]{1,4}".prop_map(|k| (k, Style::Double)),
    ]
}

fn coll<T: std::fmt::Debug + Clone>(
    item: impl Strategy<Value = T> + 'static,
) -> impl Strategy<Value = Coll<T>> {
    (
        prop::collection::vec((any::<u8>(), item), 0..7),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(items, flow, comma, multiline, compact)| Coll {
            items,
            flow,
            comma,
            multiline,
            compact,
        })
}

fn entry(inner: impl Strategy<Value = N> + 'static) -> impl Strategy<Value = (String, Style, N)> {
    (key(), inner).prop_map(|((k, st), n)| (k, st, n))
}

fn node() -> impl Strategy<Value = N> {
    scalar().prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            coll(inner.clone()).prop_map(N::Seq),
            coll(entry(inner)).prop_map(N::Map),
        ]
    })
}

fn root() -> impl Strategy<Value = N> {
    prop_oneof![
        coll(node()).prop_map(N::Seq),
        coll(entry(node())).prop_map(N::Map)
    ]
}

/// An empty item and a block scalar are only possible in block context; in flow they become
/// plain scalars.
fn normalize(n: N, in_flow: bool) -> N {
    let flat = |item: N, flow: bool| match item {
        N::Empty if flow => N::Str("e".into(), Style::Plain),
        N::Block(lines, ..) if flow => N::Str(
            if lines.is_empty() {
                "e".into()
            } else {
                lines.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ")
            },
            Style::Plain,
        ),
        item => normalize(item, flow),
    };
    match n {
        N::Seq(mut c) => {
            let flow = is_flow(&c, in_flow);
            c.items = c
                .items
                .into_iter()
                .map(|(d, item)| (d, flat(item, flow)))
                .collect();
            N::Seq(c)
        }
        N::Map(mut c) => {
            let flow = is_flow(&c, in_flow);
            c.items = c
                .items
                .into_iter()
                .map(|(d, (k, ks, item))| {
                    // A map value that is empty stays empty in flow (`{k: }`).
                    let item = match item {
                        N::Empty => N::Empty,
                        item => flat(item, flow),
                    };
                    (d, (k, ks, item))
                })
                .collect();
            N::Map(c)
        }
        n => n,
    }
}

// ---- model shape ----------------------------------------------------------------------

fn block_value(lines: &[String], folded: bool, strip: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let body = lines.join(if folded { " " } else { "\n" });
    if strip { body } else { format!("{body}\n") }
}

fn model_shape(n: &N) -> String {
    match n {
        N::Empty => "null".into(),
        N::Int(i) => i.to_string(),
        N::Str(s, _) => format!("{s:?}"),
        N::Block(lines, folded, strip) => format!("{:?}", block_value(lines, *folded, *strip)),
        N::Seq(c) => {
            let inner: Vec<String> = c.items.iter().map(|(_, n)| model_shape(n)).collect();
            format!("[{}]", inner.join(","))
        }
        N::Map(c) => {
            let inner: Vec<String> = c
                .items
                .iter()
                .enumerate()
                .map(|(i, (_, (k, _, n)))| format!("{:?}:{}", key_text(k, i), model_shape(n)))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

/// Keys are made unique by their position.
fn key_text(base: &str, i: usize) -> String {
    format!("{base}{i}")
}

// ---- rendering ------------------------------------------------------------------------

fn quote(s: &str, style: &Style) -> String {
    match style {
        Style::Plain => s.to_owned(),
        Style::Single => format!("'{}'", s.replace('\'', "''")),
        Style::Double => {
            let mut out = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
    }
}

fn scalar_text(n: &N) -> String {
    match n {
        N::Int(i) => i.to_string(),
        N::Str(s, st) => quote(s, st),
        _ => String::new(),
    }
}

/// Break a rendered scalar at its first lone space; a line break folds back into one space.
fn fold(text: &str, continuation: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    for i in 1..chars.len().saturating_sub(1) {
        if chars[i] == ' ' && chars[i - 1] != ' ' && chars[i + 1] != ' ' {
            let head: String = chars[..i].iter().collect();
            let tail: String = chars[i + 1..].iter().collect();
            return format!("{head}\n{continuation}{tail}");
        }
    }
    text.to_owned()
}

fn is_scalar(n: &N) -> bool {
    matches!(n, N::Int(_) | N::Str(..))
}

/// A collection renders as flow when asked to, when empty, or when inside flow.
fn is_flow<T>(c: &Coll<T>, in_flow: bool) -> bool {
    c.flow || c.items.is_empty() || in_flow
}

fn flow_inline(n: &N) -> String {
    match n {
        N::Seq(c) => {
            let items: Vec<String> = c.items.iter().map(|(_, n)| flow_inline(n)).collect();
            wrap('[', ']', &items, c.comma)
        }
        N::Map(c) => {
            let items: Vec<String> = c
                .items
                .iter()
                .enumerate()
                .map(|(i, (d, (k, ks, n)))| {
                    let key = quote(&key_text(k, i), ks);
                    let sep = if d & 8 != 0 { ":" } else { ": " };
                    match n {
                        N::Empty => format!("{key}{sep}"),
                        n => format!("{key}: {}", flow_inline(n)),
                    }
                })
                .collect();
            wrap('{', '}', &items, c.comma)
        }
        n => scalar_text(n),
    }
}

fn wrap(open: char, close: char, items: &[String], comma: bool) -> String {
    let tail = if comma && !items.is_empty() { "," } else { "" };
    format!("{open}{}{tail}{close}", items.join(", "))
}

const COMMENT: &str = "# note é";

/// Lines of a multi-line flow collection, starting after the opener (which the caller put on
/// the key line) and ending with the closer.
fn flow_multiline(n: &N, indent: usize) -> (char, Vec<String>) {
    let pad = " ".repeat(indent + 2);
    let (open, close, parts, comma): (char, char, Vec<(u8, String)>, bool) = match n {
        N::Seq(c) => (
            '[',
            ']',
            c.items.iter().map(|(d, n)| (*d, flow_inline(n))).collect(),
            c.comma,
        ),
        N::Map(c) => (
            '{',
            '}',
            c.items
                .iter()
                .enumerate()
                .map(|(i, (d, (k, ks, n)))| {
                    let key = quote(&key_text(k, i), ks);
                    let text = match n {
                        N::Empty => format!("{key}:"),
                        n => format!("{key}: {}", flow_inline(n)),
                    };
                    (*d, text)
                })
                .collect(),
            c.comma,
        ),
        _ => unreachable!(),
    };
    let mut lines = Vec::new();
    for (i, (d, text)) in parts.iter().enumerate() {
        if d & 2 != 0 {
            lines.push(String::new());
        }
        if d & 4 != 0 {
            lines.push(format!("{pad}{COMMENT}"));
        }
        let last = i + 1 == parts.len();
        let sep = if !last || comma { "," } else { "" };
        let note = if d & 1 != 0 {
            format!(" {COMMENT}")
        } else {
            String::new()
        };
        lines.push(format!("{pad}{text}{sep}{note}"));
    }
    lines.push(format!("{pad}{close}"));
    (open, lines)
}

fn coll_is_flow(n: &N, in_flow: bool) -> Option<bool> {
    match n {
        N::Seq(c) => Some(is_flow(c, in_flow)),
        N::Map(c) => Some(is_flow(c, in_flow)),
        _ => None,
    }
}

fn multiline(n: &N) -> bool {
    match n {
        N::Seq(c) => c.multiline && !c.items.is_empty(),
        N::Map(c) => c.multiline && !c.items.is_empty(),
        _ => false,
    }
}

/// A scalar after `prefix`, folded over two lines when the decoration bit asks for it.
fn scalar_lines(lines: &mut Vec<String>, prefix: &str, n: &N, cont_pad: &str, d: u8, note: &str) {
    let text = scalar_text(n);
    let text = if d & 16 != 0 {
        fold(&text, cont_pad)
    } else {
        text
    };
    lines.push(format!("{prefix}{text}{note}"));
}

/// A block scalar after `prefix`: header line, then its lines indented under `pad`.
fn block_scalar_lines(lines: &mut Vec<String>, prefix: &str, n: &N, pad: &str, note: &str) {
    let N::Block(content, folded, strip) = n else {
        unreachable!()
    };
    let header = format!(
        "{}{}",
        if *folded { '>' } else { '|' },
        if *strip { "-" } else { "" }
    );
    lines.push(format!("{prefix}{header}{note}"));
    lines.extend(content.iter().map(|l| format!("{pad}  {l}")));
}

/// Lines for a block collection at `indent`.
fn block(n: &N, indent: usize) -> Vec<String> {
    let pad = " ".repeat(indent);
    let mut lines = Vec::new();
    match n {
        N::Seq(c) => {
            for (i, (d, item)) in c.items.iter().enumerate() {
                if i > 0 && d & 2 != 0 {
                    lines.push(String::new());
                }
                if i > 0 && d & 4 != 0 {
                    lines.push(format!("{pad}{COMMENT}"));
                }
                let note = if d & 1 != 0 {
                    format!(" {COMMENT}")
                } else {
                    String::new()
                };
                match item {
                    N::Empty => lines.push(format!("{pad}-{note}")),
                    N::Block(..) => {
                        block_scalar_lines(&mut lines, &format!("{pad}- "), item, &pad, &note);
                    }
                    n if is_scalar(n) => {
                        scalar_lines(
                            &mut lines,
                            &format!("{pad}- "),
                            n,
                            &format!("{pad}  "),
                            *d,
                            &note,
                        );
                    }
                    n if coll_is_flow(n, false) == Some(true) => {
                        value_lines(&mut lines, &format!("{pad}- "), n, indent + 2, &note);
                    }
                    n => {
                        // Compact: the child's first line shares the dash line.
                        let mut child = block(n, indent + 2);
                        child[0] = format!("{pad}- {}", &child[0][indent + 2..]);
                        lines.extend(child);
                    }
                }
            }
        }
        N::Map(c) => {
            for (i, (d, (k, ks, item))) in c.items.iter().enumerate() {
                if i > 0 && d & 2 != 0 {
                    lines.push(String::new());
                }
                if i > 0 && d & 4 != 0 {
                    lines.push(format!("{pad}{COMMENT}"));
                }
                let key = quote(&key_text(k, i), ks);
                let note = if d & 1 != 0 {
                    format!(" {COMMENT}")
                } else {
                    String::new()
                };
                match item {
                    N::Empty => lines.push(format!("{pad}{key}:{note}")),
                    N::Block(..) => {
                        block_scalar_lines(&mut lines, &format!("{pad}{key}: "), item, &pad, &note);
                    }
                    n if is_scalar(n) => {
                        scalar_lines(
                            &mut lines,
                            &format!("{pad}{key}: "),
                            n,
                            &format!("{pad}  "),
                            *d,
                            &note,
                        );
                    }
                    n if coll_is_flow(n, false) == Some(true) => {
                        value_lines(&mut lines, &format!("{pad}{key}: "), n, indent, &note);
                    }
                    n => {
                        lines.push(format!("{pad}{key}:{note}"));
                        let compact = matches!(n, N::Seq(c) if c.compact);
                        lines.extend(block(n, if compact { indent } else { indent + 2 }));
                    }
                }
            }
        }
        _ => unreachable!(),
    }
    lines
}

/// A flow collection as a value after `prefix`, on one line or spread over several.
fn value_lines(lines: &mut Vec<String>, prefix: &str, n: &N, indent: usize, note: &str) {
    if multiline(n) {
        let (open, rest) = flow_multiline(n, indent);
        lines.push(format!("{prefix}{open}"));
        let last = rest.len() - 1;
        for (i, l) in rest.into_iter().enumerate() {
            lines.push(if i == last { format!("{l}{note}") } else { l });
        }
    } else {
        lines.push(format!("{prefix}{}{note}", flow_inline(n)));
    }
}

fn render(root: &N, nl: &str, bom: bool) -> String {
    let lines = if coll_is_flow(root, false) == Some(true) {
        let mut lines = Vec::new();
        value_lines(&mut lines, "", root, 0, "");
        lines
    } else {
        block(root, 0)
    };
    let bom = if bom { "\u{feff}" } else { "" };
    format!("{bom}---{nl}{}{nl}---{nl}body", lines.join(nl))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(
        std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(500)
    ))]

    /// Every node and key of a generated document passes the span invariants, and the parsed
    /// value equals the model that produced the text.
    #[test]
    fn generated_yaml_has_clean_spans_everywhere(
        model in root(),
        crlf in any::<bool>(),
        bom in any::<bool>(),
    ) {
        let model = normalize(model, false);
        let doc = render(&model, if crlf { "\r\n" } else { "\n" }, bom);
        let got = check(&doc);
        prop_assert_eq!(got, model_shape(&model), "{:?}", doc);
    }
}
