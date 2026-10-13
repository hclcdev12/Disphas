//! Revocation list: licenses revoked by disphas, signed by the root key and
//! shipped with every update (`docs/certificat.md` §4).
//!
//! Text form `DSR1:` + Base45 of a COSE_Sign1 signed by the root, over the
//! deterministic CBOR `{v, seq, iat, rev: [[id, at], …]}`. An attestation
//! issued **before** the revocation time of its license stays valid.
//! Revocations are permanent: lists only grow, so the union of every validly
//! signed list is always correct.

use ciborium::Value;
use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::base45;
use crate::cbor::{self, CborError, Fields, MapBuilder};
use crate::cose::{self, CoseError, Unverified};
use crate::verify::Revocation;

/// Prefix of the text form, version 1.
pub const REVOCATION_PREFIX: &str = "DSR1:";
/// Version of the list schema (`v`).
pub const REVOCATION_LIST_VERSION: u64 = 1;
/// Largest text form accepted.
pub const MAX_TEXT_BYTES: usize = 4 << 20;
/// Most entries accepted in one list.
pub const MAX_ENTRIES: usize = 100_000;

/// Why a revocation list is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RevocationError {
    /// Not an `DSR1:` text, or too large.
    #[error("not an disphas revocation list")]
    Format,
    /// The envelope is invalid or not signed by the root key.
    #[error("revocation list: {0}")]
    Signature(CoseError),
    /// The content is invalid.
    #[error("revocation list content: {0}")]
    Content(CborError),
    /// Too many entries, or an empty or duplicate license identifier.
    #[error("revocation list entries are invalid")]
    Entries,
}

/// A signed list of revoked licenses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationList {
    /// Sequence number, increased by one at every new list (`seq`).
    pub seq: u64,
    /// Signing time, UTC epoch seconds (`iat`).
    pub issued_at: u64,
    /// Revoked licenses, sorted by identifier (`rev`).
    pub entries: Vec<Revocation>,
}

impl RevocationList {
    /// Deterministic CBOR encoding. Entries are written sorted by identifier.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut entries = self.entries.clone();
        entries.sort_by(|a, b| a.license_id.cmp(&b.license_id));
        let rev = entries
            .iter()
            .map(|entry| {
                Value::Array(vec![
                    Value::Text(entry.license_id.clone()),
                    Value::Integer(entry.revoked_at.into()),
                ])
            })
            .collect();
        cbor::encode(
            MapBuilder::default()
                .field("v", Value::Integer(REVOCATION_LIST_VERSION.into()))
                .field("seq", Value::Integer(self.seq.into()))
                .field("iat", Value::Integer(self.issued_at.into()))
                .field("rev", Value::Array(rev))
                .build(),
        )
    }

    /// Strict decoding: deterministic encoding, known fields only, sorted
    /// unique identifiers.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, RevocationError> {
        let list = cbor::decode_canonical(bytes, Self::from_value, Self::to_cbor)
            .map_err(RevocationError::Content)?;
        let sorted = list
            .entries
            .windows(2)
            .all(|pair| pair[0].license_id < pair[1].license_id);
        if !sorted
            || list.entries.len() > MAX_ENTRIES
            || list.entries.iter().any(|entry| entry.license_id.is_empty())
        {
            return Err(RevocationError::Entries);
        }
        Ok(list)
    }

    fn from_value(value: Value) -> Result<Self, CborError> {
        let mut fields = Fields::of(value, "revocation list")?;
        if cbor::uint(fields.required("v")?, "v")? != REVOCATION_LIST_VERSION {
            return Err(CborError::Invalid("v"));
        }
        let seq = cbor::uint(fields.required("seq")?, "seq")?;
        let issued_at = cbor::uint(fields.required("iat")?, "iat")?;
        let entries = cbor::array(fields.required("rev")?, "rev")?
            .into_iter()
            .map(|entry| {
                let mut pair = cbor::array(entry, "rev")?.into_iter();
                let (Some(id), Some(at), None) = (pair.next(), pair.next(), pair.next()) else {
                    return Err(CborError::Invalid("rev"));
                };
                Ok(Revocation {
                    license_id: cbor::text(id, "rev")?,
                    revoked_at: cbor::uint(at, "rev")?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        fields.finish()?;
        Ok(Self {
            seq,
            issued_at,
            entries,
        })
    }

    /// Signs the list with the root key and returns its text form.
    pub fn sign(&self, root: &SigningKey) -> Result<String, CoseError> {
        let signed = cose::sign(self.to_cbor(), root)?;
        Ok(format!("{REVOCATION_PREFIX}{}", base45::encode(&signed)))
    }

    /// Decodes a text form and checks its signature by `root`.
    pub fn decode(text: &str, root: &VerifyingKey) -> Result<Self, RevocationError> {
        let text = text.trim();
        if text.len() > MAX_TEXT_BYTES {
            return Err(RevocationError::Format);
        }
        let encoded = text
            .strip_prefix(REVOCATION_PREFIX)
            .ok_or(RevocationError::Format)?;
        let signed = base45::decode(encoded).map_err(|_| RevocationError::Format)?;
        let envelope = Unverified::parse(&signed).map_err(RevocationError::Signature)?;
        let payload = envelope.verify(root).map_err(RevocationError::Signature)?;
        Self::from_cbor(payload)
    }
}

/// Union of revocation lists: the earliest time wins for a license listed
/// more than once.
pub fn merge<'a>(lists: impl IntoIterator<Item = &'a RevocationList>) -> Vec<Revocation> {
    let mut merged: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for list in lists {
        for entry in &list.entries {
            merged
                .entry(&entry.license_id)
                .and_modify(|at| *at = (*at).min(entry.revoked_at))
                .or_insert(entry.revoked_at);
        }
    }
    merged
        .into_iter()
        .map(|(id, at)| Revocation {
            license_id: id.to_owned(),
            revoked_at: at,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> SigningKey {
        SigningKey::from_bytes(&[1; 32])
    }

    fn list(seq: u64, entries: &[(&str, u64)]) -> RevocationList {
        RevocationList {
            seq,
            issued_at: 1_791_158_400,
            entries: entries
                .iter()
                .map(|(id, at)| Revocation {
                    license_id: (*id).to_owned(),
                    revoked_at: *at,
                })
                .collect(),
        }
    }

    #[test]
    fn signed_list_round_trips_sorted() {
        let text = list(3, &[("DSL-B", 20), ("DSL-A", 10)])
            .sign(&root())
            .expect("signed");
        assert!(text.starts_with(REVOCATION_PREFIX));
        let decoded = RevocationList::decode(&format!(" {text}\n"), &root().verifying_key())
            .expect("decoded");
        assert_eq!(decoded.seq, 3);
        assert_eq!(
            decoded.entries,
            list(3, &[("DSL-A", 10), ("DSL-B", 20)]).entries
        );
    }

    #[test]
    fn list_from_another_key_or_altered_is_refused() {
        let text = list(1, &[("DSL-A", 10)]).sign(&root()).expect("signed");
        let stranger = SigningKey::from_bytes(&[9; 32]).verifying_key();
        assert!(matches!(
            RevocationList::decode(&text, &stranger),
            Err(RevocationError::Signature(_))
        ));
        let mut altered = text.clone().into_bytes();
        let last = altered.len() - 3;
        altered[last] = if altered[last] == b'0' { b'1' } else { b'0' };
        assert!(
            RevocationList::decode(
                &String::from_utf8(altered).expect("ascii"),
                &root().verifying_key()
            )
            .is_err()
        );
        assert_eq!(
            RevocationList::decode("DSL1:abc", &root().verifying_key()),
            Err(RevocationError::Format)
        );
    }

    #[test]
    fn duplicate_or_empty_identifiers_are_refused() {
        for entries in [&[("DSL-A", 1), ("DSL-A", 2)][..], &[("", 1)][..]] {
            let text = list(1, entries).sign(&root()).expect("signed");
            assert_eq!(
                RevocationList::decode(&text, &root().verifying_key()),
                Err(RevocationError::Entries)
            );
        }
    }

    #[test]
    fn merge_keeps_every_license_with_its_earliest_time() {
        let merged = merge([
            &list(1, &[("DSL-A", 30)]),
            &list(2, &[("DSL-A", 10), ("DSL-B", 5)]),
        ]);
        assert_eq!(merged, list(0, &[("DSL-A", 10), ("DSL-B", 5)]).entries);
    }
}
