# frontmatter-spans

Detect YAML, TOML and JSON front matter at the start of a document and parse it. Every key and
value carries a source span, and each span indexes the whole document, not the front-matter block.

The crate does not depend on slopvac. It exists because no other crate gives front-matter values
spans: `gray_matter` has none, and `frontmatter-lossless` only finds the block.

## Use

```rust
use frontmatter_spans::FrontMatter;

let doc = "---\ntitle: Hello\n---\nBody\n";
let fm = FrontMatter::parse(doc).unwrap().unwrap();

let title = fm.get("title").unwrap();
assert_eq!(&doc[title.span.range.clone()], "Hello");
assert_eq!((title.span.start.line, title.span.start.column), (2, 8));
assert_eq!(&doc[fm.block.body_start..], "Body\n");
```

`FrontMatter::parse` returns `Ok(None)` when the document has no front matter and `Err` when a
block exists but does not parse. `detect` finds the block without parsing it.

| Format | Opens with | Closes with |
| --- | --- | --- |
| YAML | a `---` line | a `---` or `...` line |
| TOML | a `+++` line | a `+++` line |
| JSON | `{` | the matching `}` |

A leading UTF-8 byte-order mark is skipped. Lines end at LF, CRLF or a lone CR. A block that
never closes is not front matter. Trailing blanks after a delimiter are allowed.

## Spans

A `Span` holds a byte `range` and the 1-based `start` and `end` line and column. Columns count
code points, and the byte-order mark is not part of line 1. `LineIndex` gives the same mapping
for any offset in the document.

- A scalar span covers the value as written. Quoted scalars include the quotes. A block
  scalar (`|` or `>`) covers its content lines, without the header or the trailing newline.
- **YAML collection spans point at the collection's start; use child spans for extents.** A
  YAML mapping or sequence has a zero-width span at the start of its first entry: the first
  key of a mapping, or the first item of a sequence. An empty flow collection (`[]`, `{}`) is
  zero-width at its opening bracket. The reader does not guess where a YAML collection ends,
  because the parser's end positions are unreliable around comments, trailing commas and
  omitted entries.
- A YAML value that is omitted (`key:` or a bare `-`) is null with a zero-width span. It sits
  right after the `:` or `-`, or at the end of that line when a comment follows. An empty block
  scalar is the empty string, and its span is the header or the start of the next line.
- JSON and TOML collection spans are exact. JSON spans come from the crate's own parser, and
  TOML spans come from `toml_edit` with no adjustment, except that a TOML table covers its header
  and its entries.
- A TOML table span runs from its header, or its first key when it has none, to the end of its
  last entry.
- A key span covers the key as written, so JSON keys include their quotes.

## Serde

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Meta {
    genre: String,
}

let doc = "---\nslopvac:\n  genre: docs\n---\n";
let fm = frontmatter_spans::FrontMatter::parse(doc).unwrap().unwrap();
let meta = fm.deserialize_key::<Meta>("slopvac").unwrap().unwrap();
assert_eq!(meta.value.genre, "docs");
// A YAML mapping span is zero-width at its first key.
assert!(doc[meta.span.range.start..].starts_with("genre: docs"));
```

`FrontMatter::deserialize` reads the whole block. `deserialize_key` reads one top-level key and
returns the span of its value. When a value does not fit the type, `DeError::span` holds the
span of the value. For an unknown field under `deny_unknown_fields`, it holds the span of the
key.

## Parsers

- YAML uses `saphyr-parser` events, the parser behind `saphyr`. The crate builds the tree from
  the events itself, so depth is checked before anything recurses. Positions are converted from
  characters to bytes.
- TOML uses `toml_edit`, which reports byte spans for keys and values.
- JSON uses a strict RFC 8259 parser inside the crate. The common JSON crates drop spans, and a
  YAML parser would accept comments and single quotes.

### The YAML subset

The YAML reader is strict, like `StrictYAML`. This is a deliberate choice for small,
human-written metadata. It removes alias expansion attacks and tag-driven type surprises.

- **Anchors (`&x`), aliases (`*x`) and explicit tags (`!tag`, `!!str`) are errors.** The
  `ParseError` span covers the offending token, whether it sits on the same line as its node or
  on a line before it. Text such as `"&x"`, `x&y` or `hi!` is ordinary data.
- **Merge keys are plain keys.** `<<` is a key named `<<`. Its value is read like any other.
- **Keys are always strings, exactly as written.** `0x10`, `1.0`, `true`, `null` and `010` are
  the strings `"0x10"`, `"1.0"`, `"true"`, `"null"` and `"010"`. Quoted keys are unquoted.
- **Values follow the YAML 1.2 core schema.** Plain `null`, `Null`, `NULL`, `~` and an empty
  value are null. `true`, `True`, `TRUE` and the `false` forms are booleans. Integers are
  decimal with an optional sign, `0o` octal or `0x` hex. Floats include `.5`, `5.`, `1e3`,
  `.inf` and `.nan`. A decimal integer past `i64` becomes a float. Everything else, and every
  quoted or block scalar, is a string. So `yes`, `on` and `1_000` are strings.
- A non-scalar key, a duplicate key and a second YAML document in the block are errors.

TOML and JSON have no such features. A JSON object keeps every member in order, and `get`
returns the last one.

## Limits

Hostile input returns a `ParseError` and does not abort or exhaust memory. Pass a `Limits` to
`FrontMatter::parse_with` or `parse_block_with` to change the defaults.

| Limit | Default | Counts |
| --- | --- | --- |
| `max_depth` | 128 | Nested tables and arrays in YAML and JSON, root included |
| `max_input_bytes` | 16 MiB | The block content, checked before parsing |

TOML nesting is bounded by `toml_edit`. Building the line index and every span costs time
linear in the document, with a logarithmic lookup per position.

## Tests and fuzzing

```sh
cargo test -p frontmatter-spans
cargo +nightly fuzz run parse   # from crates/frontmatter-spans, needs cargo-fuzz
```

The property tests check that spans lie inside the document on character boundaries, and that
written keys and integers sit at their spans in all three formats. The limit tests cover deep
nesting, oversized blocks and a 1 MiB single-line input.

## Licence

Apache-2.0.
