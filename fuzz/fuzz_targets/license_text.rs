//! License texts installed on a workstation, and activation requests that
//! customers send to the publisher.
#![no_main]

use libfuzzer_sys::fuzz_target;
use oblivio_attestation::ed25519_dalek::SigningKey;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let root = SigningKey::from_bytes(&[1; 32]).verifying_key();
    let _ = oblivio_license::decode_license(text, &root);
    let _ = oblivio_license::read_activation_request(text);
});
