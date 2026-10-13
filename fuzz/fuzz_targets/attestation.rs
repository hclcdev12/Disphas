//! Everything a verifier reads: signed attestations, QR text, the CBOR
//! payloads (issuer certificates included), signed licenses and revocation
//! lists.
#![no_main]

use libfuzzer_sys::fuzz_target;
use disphas_attestation::ed25519_dalek::SigningKey;
use disphas_attestation::model::{ActivationRequest, Attestation, Issuer, License};
use disphas_attestation::revocation::RevocationList;
use disphas_attestation::{from_qr_text, verify, verify_license};

fuzz_target!(|data: &[u8]| {
    let root = SigningKey::from_bytes(&[1; 32]).verifying_key();
    let _ = verify(data, &root, &[]);
    let _ = Attestation::from_cbor(data);
    let _ = License::from_cbor(data);
    let _ = Issuer::from_cbor(data);
    let _ = verify_license(data, &root);
    let _ = ActivationRequest::from_cbor(data);
    let _ = RevocationList::from_cbor(data);
    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(signed) = from_qr_text(text) {
            let _ = verify(&signed, &root, &[]);
        }
        let _ = RevocationList::decode(text, &root);
    }
});
