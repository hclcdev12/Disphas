//! Requests from the UI, as decoded by the root helper: never a panic, and
//! anything accepted survives an encode/decode round trip unchanged.
#![no_main]

use libfuzzer_sys::fuzz_target;
use oblivio_protocol::Request;

fuzz_target!(|data: &[u8]| {
    let Ok(line) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(request) = Request::decode(line) {
        let encoded = request.encode().expect("an accepted request encodes");
        let again = Request::decode(&encoded).expect("its encoding decodes");
        assert_eq!(again, request);
    }
});
