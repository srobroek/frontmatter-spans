//! JSON front matter.
//!
//! No JSON crate in common use reports value spans: `serde_json` drops them and
//! `serde_json::value::RawValue` keeps only text. YAML 1.2 parsers accept JSON but also
//! accept comments, unquoted keys and single quotes, which are not JSON. A strict parser for
//! RFC 8259 is about 150 lines, so this module has its own. The fuzz target covers it.

use std::ops::Range;

use crate::{Entry, Format, Limits, LineIndex, Node, ParseError, Span, Value};

struct Parser<'a> {
    text: &'a str,
    pos: usize,
    base: usize,
    index: &'a LineIndex<'a>,
    max_depth: usize,
}

type Res<T> = Result<T, (String, usize)>;

pub(crate) fn parse(
    content: &str,
    base: usize,
    index: &LineIndex<'_>,
    limits: &Limits,
) -> Result<Node, ParseError> {
    let mut p = Parser {
        text: content,
        pos: 0,
        base,
        index,
        max_depth: limits.max_depth,
    };
    p.ws();
    let result = p.value(0).and_then(|node| {
        p.ws();
        if p.pos == content.len() {
            Ok(node)
        } else {
            Err(("trailing characters after the value".to_owned(), p.pos))
        }
    });
    result.map_err(|(message, at)| ParseError {
        format: Format::Json,
        message,
        span: Some(index.span(base + at..base + at)),
    })
}

impl Parser<'_> {
    fn span(&self, r: Range<usize>) -> Span {
        self.index.span(self.base + r.start..self.base + r.end)
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn err<T>(&self, message: &str) -> Res<T> {
        Err((message.to_owned(), self.pos))
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, b: u8, what: &str) -> Res<()> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            self.err(what)
        }
    }

    fn value(&mut self, depth: usize) -> Res<Node> {
        if matches!(self.peek(), Some(b'{' | b'[')) && depth >= self.max_depth {
            return Err((
                format!("nesting is too deep (limit {})", self.max_depth),
                self.pos,
            ));
        }
        let start = self.pos;
        let value = match self.peek() {
            Some(b'{') => self.object(depth)?,
            Some(b'[') => self.array(depth)?,
            Some(b'"') => Value::String(self.string()?),
            Some(b't') => self.literal("true", Value::Bool(true))?,
            Some(b'f') => self.literal("false", Value::Bool(false))?,
            Some(b'n') => self.literal("null", Value::Null)?,
            Some(b'-' | b'0'..=b'9') => self.number()?,
            _ => return self.err("expected a JSON value"),
        };
        Ok(Node {
            value,
            span: self.span(start..self.pos),
        })
    }

    fn literal(&mut self, word: &str, value: Value) -> Res<Value> {
        if self.text[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            self.err("expected a JSON value")
        }
    }

    fn object(&mut self, depth: usize) -> Res<Value> {
        self.pos += 1;
        let mut entries = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Table(entries));
        }
        loop {
            self.ws();
            let key_start = self.pos;
            if self.peek() != Some(b'"') {
                return self.err("expected a string key");
            }
            let key = self.string()?;
            let key_span = self.span(key_start..self.pos);
            self.ws();
            self.expect(b':', "expected `:` after the key")?;
            self.ws();
            let value = self.value(depth + 1)?;
            entries.push(Entry {
                key,
                key_span,
                value,
            });
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Table(entries));
                }
                _ => return self.err("expected `,` or `}`"),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Res<Value> {
        self.pos += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return self.err("expected `,` or `]`"),
            }
        }
    }

    fn number(&mut self) -> Res<Value> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return self.err("expected a digit"),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !self.peek().is_some_and(|b| b.is_ascii_digit()) {
                return self.err("expected a digit after `.`");
            }
            self.digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !self.peek().is_some_and(|b| b.is_ascii_digit()) {
                return self.err("expected a digit in the exponent");
            }
            self.digits();
        }
        let text = &self.text[start..self.pos];
        if !is_float && let Ok(i) = text.parse::<i64>() {
            return Ok(Value::Integer(i));
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| ("number out of range".to_owned(), start))
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
    }

    fn string(&mut self) -> Res<String> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.text[self.pos..].chars().next() else {
                return self.err("unterminated string");
            };
            match c {
                '"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                '\\' => {
                    self.pos += 1;
                    out.push(self.escape()?);
                }
                c if c < '\u{20}' => return self.err("control character in string"),
                c => {
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
    }

    fn escape(&mut self) -> Res<char> {
        let Some(b) = self.peek() else {
            return self.err("unterminated escape");
        };
        self.pos += 1;
        Ok(match b {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.unicode(),
            _ => {
                self.pos -= 1;
                return self.err("invalid escape");
            }
        })
    }

    fn hex4(&mut self) -> Res<u32> {
        let digits = self.text.get(self.pos..self.pos + 4);
        match digits.and_then(|d| u32::from_str_radix(d, 16).ok()) {
            Some(v) if digits.is_some_and(|d| d.bytes().all(|b| b.is_ascii_hexdigit())) => {
                self.pos += 4;
                Ok(v)
            }
            _ => self.err("expected four hex digits"),
        }
    }

    fn unicode(&mut self) -> Res<char> {
        let first = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&first) {
            if !self.text[self.pos..].starts_with("\\u") {
                return self.err("unpaired surrogate");
            }
            self.pos += 2;
            let second = self.hex4()?;
            if !(0xDC00..0xE000).contains(&second) {
                return self.err("unpaired surrogate");
            }
            0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
        } else {
            first
        };
        char::from_u32(code).map_or_else(|| self.err("unpaired surrogate"), Ok)
    }
}
