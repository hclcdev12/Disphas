//! Deterministic CBOR (RFC 8949 §4.2) and strict map reading.
//!
//! Strictness comes from one rule: a decoded structure must re-encode to the
//! exact same bytes. Unknown or duplicate keys, `null` instead of an absent
//! field, unsorted keys, long-form integers and trailing bytes all fail it.

use ciborium::Value;
use thiserror::Error;

/// Malformed or non-canonical CBOR content.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CborError {
    /// Not valid CBOR.
    #[error("invalid CBOR: {0}")]
    Syntax(String),
    /// A required field is missing.
    #[error("missing field {0:?}")]
    Missing(&'static str),
    /// A field has the wrong type or an invalid value.
    #[error("invalid field {0:?}")]
    Invalid(&'static str),
    /// A field this version does not know.
    #[error("unknown field")]
    Unknown,
    /// Valid CBOR, but not in deterministic form.
    #[error("not in deterministic encoding")]
    NotCanonical,
    /// Well-formed fields that contradict each other or the certificate
    /// rules, such as a host write certified Purge.
    #[error("inconsistent attestation: {0}")]
    Inconsistent(&'static str),
}

/// Encodes `value` deterministically: map keys sorted by their encoded bytes.
pub(crate) fn encode(value: Value) -> Vec<u8> {
    let mut out = Vec::new();
    // Writing into a Vec cannot fail.
    let _ = ciborium::ser::into_writer(&canonical(value), &mut out);
    out
}

/// Decodes one CBOR item, then requires `bytes` to be its deterministic encoding.
pub(crate) fn decode_canonical<T>(
    bytes: &[u8],
    parse: impl FnOnce(Value) -> Result<T, CborError>,
    encode_back: impl FnOnce(&T) -> Vec<u8>,
) -> Result<T, CborError> {
    let value: Value =
        ciborium::de::from_reader(bytes).map_err(|error| CborError::Syntax(error.to_string()))?;
    let decoded = parse(value)?;
    if encode_back(&decoded) != bytes {
        return Err(CborError::NotCanonical);
    }
    Ok(decoded)
}

fn canonical(value: Value) -> Value {
    match value {
        Value::Map(entries) => {
            let mut entries: Vec<(Vec<u8>, Value, Value)> = entries
                .into_iter()
                .map(|(key, value)| {
                    let key = canonical(key);
                    let mut encoded = Vec::new();
                    let _ = ciborium::ser::into_writer(&key, &mut encoded);
                    (encoded, key, canonical(value))
                })
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(
                entries
                    .into_iter()
                    .map(|(_, key, value)| (key, value))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonical).collect()),
        Value::Tag(tag, inner) => Value::Tag(tag, Box::new(canonical(*inner))),
        other => other,
    }
}

/// Builds a map with text keys, skipping absent optional fields.
#[derive(Default)]
pub(crate) struct MapBuilder(Vec<(Value, Value)>);

impl MapBuilder {
    pub(crate) fn field(mut self, key: &str, value: Value) -> Self {
        self.0.push((Value::Text(key.to_owned()), value));
        self
    }

    pub(crate) fn optional(self, key: &str, value: Option<Value>) -> Self {
        match value {
            Some(value) => self.field(key, value),
            None => self,
        }
    }

    pub(crate) fn build(self) -> Value {
        Value::Map(self.0)
    }
}

/// Reads the fields of a map with text keys; every field must be consumed.
pub(crate) struct Fields(Vec<(Value, Value)>);

impl Fields {
    pub(crate) fn of(value: Value, name: &'static str) -> Result<Self, CborError> {
        match value {
            Value::Map(entries) => Ok(Self(entries)),
            _ => Err(CborError::Invalid(name)),
        }
    }

    pub(crate) fn optional(&mut self, key: &'static str) -> Option<Value> {
        let index = self
            .0
            .iter()
            .position(|(candidate, _)| candidate.as_text() == Some(key))?;
        Some(self.0.remove(index).1)
    }

    pub(crate) fn required(&mut self, key: &'static str) -> Result<Value, CborError> {
        self.optional(key).ok_or(CborError::Missing(key))
    }

    /// Fails if any field was not consumed.
    pub(crate) fn finish(self) -> Result<(), CborError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(CborError::Unknown)
        }
    }
}

pub(crate) fn text(value: Value, name: &'static str) -> Result<String, CborError> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(CborError::Invalid(name)),
    }
}

pub(crate) fn uint(value: Value, name: &'static str) -> Result<u64, CborError> {
    match value {
        Value::Integer(integer) => u64::try_from(integer).map_err(|_| CborError::Invalid(name)),
        _ => Err(CborError::Invalid(name)),
    }
}

pub(crate) fn boolean(value: Value, name: &'static str) -> Result<bool, CborError> {
    match value {
        Value::Bool(flag) => Ok(flag),
        _ => Err(CborError::Invalid(name)),
    }
}

pub(crate) fn bytes(value: Value, name: &'static str) -> Result<Vec<u8>, CborError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(CborError::Invalid(name)),
    }
}

pub(crate) fn fixed_bytes<const N: usize>(
    value: Value,
    name: &'static str,
) -> Result<[u8; N], CborError> {
    bytes(value, name)?
        .try_into()
        .map_err(|_| CborError::Invalid(name))
}

pub(crate) fn array(value: Value, name: &'static str) -> Result<Vec<Value>, CborError> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(CborError::Invalid(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_keys_are_sorted_by_encoded_bytes() {
        // Shorter text keys sort first, then bytewise.
        let value = MapBuilder::default()
            .field("tool", Value::Bool(true))
            .field("v", Value::Bool(true))
            .field("id", Value::Bool(true))
            .field("iat", Value::Bool(true))
            .build();
        let encoded = encode(value);
        let decoded: Value = ciborium::de::from_reader(&encoded[..]).expect("valid CBOR");
        let keys: Vec<&str> = decoded
            .as_map()
            .expect("map")
            .iter()
            .filter_map(|(key, _)| key.as_text())
            .collect();
        assert_eq!(keys, ["v", "id", "iat", "tool"]);
    }

    #[test]
    fn integers_use_the_shortest_form() {
        assert_eq!(encode(Value::Integer(23.into())), [0x17]);
        assert_eq!(encode(Value::Integer(24.into())), [0x18, 24]);
        assert_eq!(encode(Value::Integer(1000.into())), [0x19, 0x03, 0xe8]);
    }
}
