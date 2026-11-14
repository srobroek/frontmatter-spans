//! Property tests: spans stay inside the document and name the text they claim to.

use std::collections::BTreeMap;

use frontmatter_spans::{FrontMatter, Node, Value};
use proptest::prelude::*;

fn check_spans(doc: &str, node: &Node) {
    assert!(node.span.range.start <= node.span.range.end);
    assert!(node.span.range.end <= doc.len());
    assert!(doc.is_char_boundary(node.span.range.start));
    assert!(doc.is_char_boundary(node.span.range.end));
    match &node.value {
        Value::Table(entries) => {
            for e in entries {
                assert!(e.key_span.range.end <= doc.len());
                assert!(doc.is_char_boundary(e.key_span.range.start));
                assert!(doc.is_char_boundary(e.key_span.range.end));
                check_spans(doc, &e.value);
            }
        }
        Value::Array(items) => items.iter().for_each(|n| check_spans(doc, n)),
        _ => {}
    }
}

proptest! {
    /// No input panics, and any span that comes back lies on character boundaries in the document.
    #[test]
    fn arbitrary_documents_never_panic_and_spans_stay_in_bounds(doc in "(\\PC|\\n|\\r|---|\\+\\+\\+|\\{|\\}|\\[|\\]|\"|: |\\.\\.\\.){0,40}") {
        if let Ok(Some(fm)) = FrontMatter::parse(&doc) {
            check_spans(&doc, &fm.root);
            prop_assert!(fm.block.body_start <= doc.len());
            prop_assert!(fm.block.content.end <= doc.len());
        }
    }

    /// The same map written as YAML, TOML and JSON, with any prefix and line ending, gives
    /// spans that slice back to the key and to the integer as written.
    #[test]
    fn written_keys_and_integers_are_found_at_their_spans(
        map in prop::collection::btree_map("[a-z]{1,8}", -1000i64..1000, 1..6),
        bom in any::<bool>(),
        nl in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        which in 0usize..3,
    ) {
        prop_assume!(!(which == 1 && nl == "\r")); // TOML forbids a lone CR
        let doc = render(&map, bom, nl, which);
        let fm = FrontMatter::parse(&doc).unwrap().expect("block");
        let Value::Table(entries) = &fm.root.value else { panic!("not a table") };
        prop_assert_eq!(entries.len(), map.len());
        for e in entries {
            let quote = if which == 2 { "\"" } else { "" };
            prop_assert_eq!(&doc[e.key_span.range.clone()], format!("{quote}{}{quote}", e.key));
            let want = map[&e.key];
            prop_assert_eq!(&e.value.value, &Value::Integer(want));
            prop_assert_eq!(&doc[e.value.span.range.clone()], want.to_string());
        }
        prop_assert!(doc[fm.block.body_start..].starts_with("body"));
    }
}

fn render(map: &BTreeMap<String, i64>, bom: bool, nl: &str, which: usize) -> String {
    let lines: Vec<String> = match which {
        0 => [
            vec!["---".to_owned()],
            map.iter().map(|(k, v)| format!("{k}: {v}")).collect(),
            vec!["---".to_owned()],
        ]
        .concat(),
        1 => [
            vec!["+++".to_owned()],
            map.iter().map(|(k, v)| format!("{k} = {v}")).collect(),
            vec!["+++".to_owned()],
        ]
        .concat(),
        _ => {
            let members: Vec<String> = map.iter().map(|(k, v)| format!("  \"{k}\": {v}")).collect();
            vec![format!("{{{nl}{}{nl}}}", members.join(&format!(",{nl}")))]
        }
    };
    let bom = if bom { "\u{feff}" } else { "" };
    format!("{bom}{}{nl}body", lines.join(nl))
}
