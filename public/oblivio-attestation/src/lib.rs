//! Oblivio attestation: schema, QR/CBOR/COSE decoding, signature and
//! certificate-chain verification.
//!
//! The attestation contains codes only, never translated text.
//! See `docs/certificat.md`.
//!
//! Depends on nothing from the private Oblivio code base: this crate is
//! published as is, together with `oblivio-verify`.

pub mod base45;
pub mod cbor;
pub mod cose;
pub mod display;
pub mod model;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod trust;
pub mod verify;

/// Re-exported so that users share the exact Ed25519 implementation.
pub use ed25519_dalek;
pub use model::{Attestation, License};
pub use verify::{
    Revocation, Verified, VerifyError, from_qr_text, sign_attestation, to_qr_text, verify,
};

#[cfg(test)]
mod tests;
