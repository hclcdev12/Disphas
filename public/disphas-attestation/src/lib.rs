//! disphas attestation: schema, QR/CBOR/COSE decoding, signature and
//! certificate-chain verification.
//!
//! The attestation contains codes only, never translated text.
//! See `docs/certificat.md`.
//!
//! Depends on nothing from the private disphas code base: this crate is
//! published as is, together with `disphas-verify`.

pub mod base45;
pub mod batch;
pub mod cbor;
pub mod cose;
pub mod display;
pub mod model;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod revocation;
pub mod trust;
pub mod verify;

/// Re-exported so that users share the exact Ed25519 implementation.
pub use ed25519_dalek;
pub use model::{Attestation, Issuer, License};
pub use verify::{
    Revocation, Verified, VerifyError, from_qr_text, is_revoked, sign_attestation, to_qr_text,
    verify, verify_license,
};

#[cfg(test)]
mod tests;
