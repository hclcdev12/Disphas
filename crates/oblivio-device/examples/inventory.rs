//! Prints the read-only inventory of this machine: `cargo run -p oblivio-device --example inventory`.
//!
//! Reads `/sys` and `/proc` (and lists a system automount point to trigger it).
//! Needs no privileges and touches no drive.

use std::process::ExitCode;

fn main() -> ExitCode {
    match oblivio_device::scan(&oblivio_device::SystemPaths::default()) {
        Ok(devices) => {
            for device in devices {
                println!("{device:#?}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("inventory failed: {error}");
            ExitCode::FAILURE
        }
    }
}
