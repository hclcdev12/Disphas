//! Data returned by drives (IDENTIFY, logs, SMART): a faulty or hostile
//! firmware must never crash the root helper.
#![no_main]

use libfuzzer_sys::fuzz_target;
use disphas_device::decode::{ata, nvme};

fuzz_target!(|data: &[u8]| {
    let _ = nvme::identify_controller(data);
    let _ = nvme::identify_namespace(data);
    let _ = nvme::health(data);
    let _ = nvme::sanitize_state(data);
    let _ = ata::identify_device(data);
    let _ = ata::smart_attributes(data);
});
