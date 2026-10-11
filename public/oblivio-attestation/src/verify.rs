//! Signing, QR encoding and offline verification of the trust chain
//! (`docs/certificat.md` §1, §4, §5).
//!
//! ```text
//! root key ──signs──► license v1 { workstation key } ──signs──► attestation
//! root key ──signs──► issuer { shop key } ──signs──► license v2 ──signs──► attestation
//! ```

use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::base45;
use crate::cbor::CborError;
use crate::cose::{self, CoseError, Unverified};
use crate::model::{Attestation, Issuer, License};

/// Prefix of the QR text, version 1.
pub const QR_PREFIX: &str = "OBV1:";
/// Largest decompressed attestation accepted from a QR code.
pub const MAX_ATTESTATION_BYTES: usize = 64 * 1024;

/// A license, or every license of an issuer, revoked at a given time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    /// Revoked license identifier, or issuer identifier (prefix `ISS-`).
    pub license_id: String,
    /// Revocation time, UTC epoch seconds.
    pub revoked_at: u64,
}

/// Why an attestation is not authentic.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerifyError {
    /// The QR text is not an Oblivio attestation.
    #[error("not an Oblivio v1 QR code")]
    NotOblivio,
    /// The QR text is corrupted.
    #[error("corrupted QR content")]
    Corrupted,
    /// The attestation envelope is invalid.
    #[error("attestation: {0}")]
    AttestationEnvelope(CoseError),
    /// The attestation content is invalid.
    #[error("attestation content: {0}")]
    AttestationContent(CborError),
    /// The license, or its issuer certificate, is invalid or does not chain
    /// to the root key.
    #[error("license: {0}")]
    License(CoseError),
    /// The license or issuer content is invalid, or the issuer was not
    /// allowed to sign this license.
    #[error("license content: {0}")]
    LicenseContent(CborError),
    /// The workstation key in the license is not a valid Ed25519 key.
    #[error("invalid workstation key")]
    WorkstationKey,
    /// The license was not valid when the attestation was issued.
    #[error("license not valid at the attestation date")]
    LicenseNotValid,
    /// The license, or its issuer, was revoked before the attestation was issued.
    #[error("license revoked before the attestation date")]
    LicenseRevoked,
    /// The attestation is not signed by the licensed workstation.
    #[error("attestation: {0}")]
    AttestationSignature(CoseError),
}

/// An attestation whose whole chain was verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// The attestation.
    pub attestation: Attestation,
    /// The license of the workstation that signed it.
    pub license: License,
    /// The issuer that signed the license, for a version 2 license.
    pub issuer: Option<Issuer>,
}

impl Verified {
    /// Whether `journal` is the detailed journal this attestation refers to.
    pub fn matches_journal(&self, journal: &[u8]) -> bool {
        Sha256::digest(journal).as_slice() == self.attestation.journal_sha256
    }
}

/// Signs an attestation with the workstation key: a tagged COSE_Sign1.
pub fn sign_attestation(attestation: &Attestation, key: &SigningKey) -> Result<Vec<u8>, CoseError> {
    cose::sign(attestation.to_cbor(), key)
}

/// QR text of a signed attestation: prefix, then Base45 of the zlib-compressed COSE_Sign1.
pub fn to_qr_text(signed: &[u8]) -> String {
    format!(
        "{QR_PREFIX}{}",
        base45::encode(&miniz_oxide::deflate::compress_to_vec_zlib(signed, 9))
    )
}

/// Signed attestation carried by a QR text.
pub fn from_qr_text(text: &str) -> Result<Vec<u8>, VerifyError> {
    let encoded = text
        .strip_prefix(QR_PREFIX)
        .ok_or(VerifyError::NotOblivio)?;
    let compressed = base45::decode(encoded).map_err(|_| VerifyError::Corrupted)?;
    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&compressed, MAX_ATTESTATION_BYTES)
        .map_err(|_| VerifyError::Corrupted)
}

/// Authenticates a signed license against the root public key alone: signed
/// by the root (version 1), or by the shop key of its embedded issuer
/// certificate, itself signed by the root and allowed to sign this license
/// (version 2). Validity dates and revocations are the caller's to check.
pub fn verify_license(
    signed: &[u8],
    root: &VerifyingKey,
) -> Result<(License, Option<Issuer>), VerifyError> {
    let envelope = Unverified::parse(signed).map_err(VerifyError::License)?;
    // Parsed before its signature is checked, only to find the signer.
    let claimed =
        License::from_cbor(envelope.untrusted_payload()).map_err(VerifyError::LicenseContent)?;
    let Some(issuer_signed) = &claimed.issuer else {
        envelope.verify(root).map_err(VerifyError::License)?;
        return Ok((claimed, None));
    };
    let issuer_envelope = Unverified::parse(issuer_signed).map_err(VerifyError::License)?;
    let issuer_payload = issuer_envelope.verify(root).map_err(VerifyError::License)?;
    let issuer = Issuer::from_cbor(issuer_payload).map_err(VerifyError::LicenseContent)?;
    let shop = VerifyingKey::from_bytes(&issuer.key)
        .map_err(|_| VerifyError::LicenseContent(CborError::Invalid("ipk")))?;
    envelope.verify(&shop).map_err(VerifyError::License)?;
    if !issuer.allows(&claimed) {
        return Err(VerifyError::LicenseContent(CborError::Inconsistent(
            "license outside the bounds of its issuer",
        )));
    }
    Ok((claimed, Some(issuer)))
}

/// Whether `license`, or its `issuer`, was revoked at or before `time`.
pub fn is_revoked(
    license: &License,
    issuer: Option<&Issuer>,
    revocations: &[Revocation],
    time: u64,
) -> bool {
    revocations.iter().any(|revocation| {
        revocation.revoked_at <= time
            && (revocation.license_id == license.id
                || issuer.is_some_and(|issuer| revocation.license_id == issuer.id))
    })
}

/// Verifies a signed attestation against the root public key alone.
///
/// Checks, in order: the attestation envelope, its content, the license
/// chain to the root key ([`verify_license`]), the license validity and
/// revocation (license or issuer) at the attestation date, and finally the
/// attestation signature by the licensed workstation key. A revocation dated
/// after the attestation does not invalidate it.
pub fn verify(
    signed: &[u8],
    root: &VerifyingKey,
    revocations: &[Revocation],
) -> Result<Verified, VerifyError> {
    let envelope = Unverified::parse(signed).map_err(VerifyError::AttestationEnvelope)?;
    // Parsed before its signature is checked, only to find the license.
    let claimed = Attestation::from_cbor(envelope.untrusted_payload())
        .map_err(VerifyError::AttestationContent)?;

    let (license, issuer) = verify_license(&claimed.license, root)?;
    if !license.is_valid_at(claimed.issued_at) {
        return Err(VerifyError::LicenseNotValid);
    }
    if is_revoked(&license, issuer.as_ref(), revocations, claimed.issued_at) {
        return Err(VerifyError::LicenseRevoked);
    }

    let workstation = VerifyingKey::from_bytes(&license.workstation_key)
        .map_err(|_| VerifyError::WorkstationKey)?;
    envelope
        .verify(&workstation)
        .map_err(VerifyError::AttestationSignature)?;
    Ok(Verified {
        attestation: claimed,
        license,
        issuer,
    })
}
