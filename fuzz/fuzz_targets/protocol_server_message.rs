//! Responses and events from the helper, as decoded by the app.
#![no_main]

use libfuzzer_sys::fuzz_target;
use disphas_protocol::ServerMessage;

fuzz_target!(|data: &[u8]| {
    if let Ok(line) = std::str::from_utf8(data)
        && let Ok(message) = ServerMessage::decode(line)
    {
        let encoded = message.encode().expect("an accepted message encodes");
        ServerMessage::decode(&encoded).expect("its encoding decodes");
    }
});
