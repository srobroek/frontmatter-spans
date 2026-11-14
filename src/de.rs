//! A serde `Deserializer` over a parsed [`Node`].
//!
//! Errors carry the span of the innermost node or key at fault.

use serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};

use crate::{DeError, Entry, Node, Value};

pub(crate) struct NodeDe<'de>(pub(crate) &'de Node);

impl<'de> de::Deserializer<'de> for NodeDe<'de> {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.visit(visitor).map_err(|e| e.or_span(&self.0.span))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        if self.0.value == Value::Null {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        let node = self.0;
        match &node.value {
            Value::String(s) => visitor.visit_enum(s.as_str().into_deserializer()),
            Value::Table(entries) if entries.len() == 1 => visitor.visit_enum(Variant(&entries[0])),
            _ => Err(DeError {
                message: "expected a string or a single-key table for an enum".to_owned(),
                span: None,
            }),
        }
        .map_err(|e| e.or_span(&node.span))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes
        byte_buf unit unit_struct seq tuple tuple_struct map struct identifier
        ignored_any
    }
}

impl<'de> NodeDe<'de> {
    /// Drive `visitor` over the node. Errors may still lack a span; the caller adds one.
    fn visit<V: Visitor<'de>>(&self, visitor: V) -> Result<V::Value, DeError> {
        let node = self.0;
        match &node.value {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(*b),
            Value::Integer(i) => visitor.visit_i64(*i),
            Value::Float(f) => visitor.visit_f64(*f),
            Value::String(s) | Value::Datetime(s) => visitor.visit_borrowed_str(s),
            Value::Array(items) => {
                let mut seq = Seq(items.iter());
                let value = visitor.visit_seq(&mut seq)?;
                match seq.0.next() {
                    // A visitor that stops early (a tuple, a fixed array) left items behind.
                    Some(extra) => Err(DeError {
                        message: format!(
                            "invalid length {}, expected fewer elements in array",
                            items.len()
                        ),
                        span: Some(extra.span.clone()),
                    }),
                    None => Ok(value),
                }
            }
            Value::Table(entries) => {
                let mut map = Map {
                    entries: entries.iter(),
                    pending: None,
                };
                let value = visitor.visit_map(&mut map)?;
                match map.entries.next() {
                    Some(extra) => Err(DeError {
                        message: format!(
                            "invalid length {}, expected fewer entries in table",
                            entries.len()
                        ),
                        span: Some(extra.key_span.clone()),
                    }),
                    None => Ok(value),
                }
            }
        }
    }
}

struct Seq<'de>(std::slice::Iter<'de, Node>);

impl<'de> SeqAccess<'de> for Seq<'de> {
    type Error = DeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, DeError> {
        self.0
            .next()
            .map(|n| seed.deserialize(NodeDe(n)).map_err(|e| e.or_span(&n.span)))
            .transpose()
    }
}

struct Map<'de> {
    entries: std::slice::Iter<'de, Entry>,
    pending: Option<&'de Node>,
}

impl<'de> MapAccess<'de> for Map<'de> {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, DeError> {
        let Some(entry) = self.entries.next() else {
            return Ok(None);
        };
        self.pending = Some(&entry.value);
        seed.deserialize(entry.key.as_str().into_deserializer())
            .map(Some)
            .map_err(|e: DeError| e.or_span(&entry.key_span))
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, DeError> {
        let node = self
            .pending
            .take()
            .ok_or_else(|| de::Error::custom("value requested before key"))?;
        seed.deserialize(NodeDe(node))
            .map_err(|e| e.or_span(&node.span))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len())
    }
}

struct Variant<'de>(&'de Entry);

impl<'de> EnumAccess<'de> for Variant<'de> {
    type Error = DeError;
    type Variant = NodeDe<'de>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, NodeDe<'de>), DeError> {
        let name = seed
            .deserialize(self.0.key.as_str().into_deserializer())
            .map_err(|e: DeError| e.or_span(&self.0.key_span))?;
        Ok((name, NodeDe(&self.0.value)))
    }
}

impl<'de> VariantAccess<'de> for NodeDe<'de> {
    type Error = DeError;

    fn unit_variant(self) -> Result<(), DeError> {
        de::Deserialize::deserialize(self)
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, DeError> {
        let span = self.0.span.clone();
        seed.deserialize(self).map_err(|e| e.or_span(&span))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, DeError> {
        de::Deserializer::deserialize_seq(self, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        de::Deserializer::deserialize_map(self, visitor)
    }
}
