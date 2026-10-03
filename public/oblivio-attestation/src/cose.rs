//! COSE_Sign1 (RFC 9052) with EdDSA over Ed25519, and nothing else.

use coset::{CoseSign1, CoseSign1Builder, HeaderBuilder, TaggedCborSerializable, iana};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

/// A COSE_Sign1 message that cannot be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoseError {
    /// Not a tagged COSE_Sign1 structure.
    #[error("malformed COSE_Sign1: {0}")]
    Malformed(String),
    /// Headers other than `alg: EdDSA`, or an unprotected header.
    #[error("unexpected COSE headers: only alg EdDSA is accepted")]
    UnexpectedHeaders,
    /// No payload.
    #[error("COSE_Sign1 without payload")]
    MissingPayload,
    /// The message could not be serialized.
    #[error("cannot serialize COSE_Sign1: {0}")]
    Serialize(String),
    /// The signature does not verify.
    #[error("invalid signature")]
    BadSignature,
}

fn expected_protected() -> coset::Header {
    HeaderBuilder::new()
        .algorithm(iana::Algorithm::EdDSA)
        .build()
}

/// Signs `payload` into a tagged COSE_Sign1 message.
pub fn sign(payload: Vec<u8>, key: &SigningKey) -> Result<Vec<u8>, CoseError> {
    CoseSign1Builder::new()
        .protected(expected_protected())
        .payload(payload)
        .create_signature(b"", |data| key.sign(data).to_bytes().to_vec())
        .build()
        .to_tagged_vec()
        .map_err(|error| CoseError::Serialize(format!("{error:?}")))
}

/// A parsed COSE_Sign1 message whose signature is not verified yet.
#[derive(Debug, Clone)]
pub struct Unverified {
    message: CoseSign1,
}

impl Unverified {
    /// Parses a tagged COSE_Sign1 message, accepting only `alg: EdDSA`.
    pub fn parse(bytes: &[u8]) -> Result<Self, CoseError> {
        let message = CoseSign1::from_tagged_slice(bytes)
            .map_err(|error| CoseError::Malformed(format!("{error:?}")))?;
        if message.protected.header != expected_protected()
            || message.unprotected != coset::Header::default()
        {
            return Err(CoseError::UnexpectedHeaders);
        }
        if message.payload.is_none() {
            return Err(CoseError::MissingPayload);
        }
        Ok(Self { message })
    }

    /// The payload, not yet authenticated: parse it, never trust it.
    pub fn untrusted_payload(&self) -> &[u8] {
        self.message.payload.as_deref().unwrap_or_default()
    }

    /// Verifies the signature with `key` and returns the authenticated payload.
    pub fn verify(&self, key: &VerifyingKey) -> Result<&[u8], CoseError> {
        self.message.verify_signature(b"", |signature, data| {
            let signature =
                Signature::from_slice(signature).map_err(|_| CoseError::BadSignature)?;
            key.verify_strict(data, &signature)
                .map_err(|_| CoseError::BadSignature)
        })?;
        Ok(self.untrusted_payload())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coset::CborSerializable;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    #[test]
    fn signs_and_verifies() {
        let message = sign(b"payload".to_vec(), &key(1)).expect("signed");
        let parsed = Unverified::parse(&message).expect("valid COSE_Sign1");
        assert_eq!(parsed.verify(&key(1).verifying_key()), Ok(&b"payload"[..]));
    }

    #[test]
    fn wrong_key_is_rejected() {
        let message = sign(b"payload".to_vec(), &key(1)).expect("signed");
        let parsed = Unverified::parse(&message).expect("valid COSE_Sign1");
        assert_eq!(
            parsed.verify(&key(2).verifying_key()),
            Err(CoseError::BadSignature)
        );
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let mut message = sign(b"payload".to_vec(), &key(1)).expect("signed");
        let position = message
            .windows(7)
            .position(|window| window == b"payload")
            .expect("payload in clear");
        message[position] = b'P';
        let parsed = Unverified::parse(&message).expect("still well-formed");
        assert_eq!(
            parsed.verify(&key(1).verifying_key()),
            Err(CoseError::BadSignature)
        );
    }

    #[test]
    fn other_algorithms_are_rejected() {
        let message = CoseSign1Builder::new()
            .protected(
                HeaderBuilder::new()
                    .algorithm(iana::Algorithm::ES256)
                    .build(),
            )
            .payload(b"payload".to_vec())
            .signature(vec![0; 64])
            .build()
            .to_tagged_vec()
            .expect("serializable");
        assert_eq!(
            Unverified::parse(&message).err(),
            Some(CoseError::UnexpectedHeaders)
        );
    }

    #[test]
    fn untagged_message_is_rejected() {
        let message = CoseSign1Builder::new()
            .protected(expected_protected())
            .payload(b"payload".to_vec())
            .create_signature(b"", |data| key(1).sign(data).to_bytes().to_vec())
            .build()
            .to_vec()
            .expect("serializable");
        assert!(matches!(
            Unverified::parse(&message),
            Err(CoseError::Malformed(_))
        ));
    }
}
