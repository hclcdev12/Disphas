//! Development root key: **not secret**, published in this file on purpose.
//!
//! It lets developers issue licenses to themselves and exercise the whole
//! chain without the official root key. It only exists in builds with debug
//! assertions: release builds do not contain it and cannot accept licenses
//! it signed. Certificates signed under it are worthless by construction.

use disphas_attestation::License;
use disphas_attestation::ed25519_dalek::{SigningKey, VerifyingKey};

use crate::{LicenseError, license_text, read_activation_request};

/// Seed of the development root key. Public: never use it for anything else.
const SEED: [u8; 32] = *b"DISPHAS DEV ROOT - NOT A SECRET!";

/// The development root signing key.
pub fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&SEED)
}

/// The development root public key.
pub fn verifying_key() -> VerifyingKey {
    signing_key().verifying_key()
}

/// Issues a development license (`DSL1:…`) for an activation request,
/// valid from `now` for `days` days, with the `Dev` offer.
pub fn issue(request: &str, now: u64, days: u64) -> Result<String, LicenseError> {
    issue_for_offer(request, now, days, "Dev")
}

/// Same as [`issue`], with another offer: tests of the entitlements of each
/// offer (`docs/offres.md` §4).
pub fn issue_for_offer(
    request: &str,
    now: u64,
    days: u64,
    offer: &str,
) -> Result<String, LicenseError> {
    let request = read_activation_request(request)?;
    let license = License {
        id: format!("DEV-{now}"),
        customer: "disphas development".to_owned(),
        offer: offer.to_owned(),
        not_before: now.saturating_sub(60),
        expires: Some(now.saturating_add(days.saturating_mul(86_400))),
        workstation_key: request.workstation_key,
        issuer: None,
    };
    let signed = disphas_attestation::cose::sign(license.to_cbor(), &signing_key())
        .map_err(|_| LicenseError::Signature)?;
    Ok(license_text(&signed))
}
