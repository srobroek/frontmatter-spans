#![no_main]
//! Feeds arbitrary text to the detector and the three parsers and checks that spans stay valid.

use frontmatter_spans::{FrontMatter, Node, Value};
use libfuzzer_sys::fuzz_target;
use serde::de::IgnoredAny;

fn check(doc: &str, node: &Node) {
    let r = &node.span.range;
    assert!(r.start <= r.end && r.end <= doc.len());
    assert!(doc.is_char_boundary(r.start) && doc.is_char_boundary(r.end));
    match &node.value {
        Value::Table(entries) => {
            for e in entries {
                let k = &e.key_span.range;
                assert!(k.start <= k.end && k.end <= doc.len());
                assert!(doc.is_char_boundary(k.start) && doc.is_char_boundary(k.end));
                check(doc, &e.value);
            }
        }
        Value::Array(items) => items.iter().for_each(|n| check(doc, n)),
        _ => {}
    }
}

fuzz_target!(|doc: &str| {
    if let Ok(Some(fm)) = FrontMatter::parse(doc) {
        assert!(fm.block.content.end <= fm.block.outer.end);
        assert!(fm.block.body_start <= doc.len());
        check(doc, &fm.root);
        let _ = fm.deserialize::<IgnoredAny>();
    }
});
