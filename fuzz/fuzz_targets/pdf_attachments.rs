//! Certificate PDFs given to the app or to `oblivio-verify`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = oblivio_attestation::pdf::attachments(data);
});
