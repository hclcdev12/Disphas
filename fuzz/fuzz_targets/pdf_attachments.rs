//! Certificate PDFs given to the app or to `disphas-verify`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = disphas_attestation::pdf::attachments(data);
});
