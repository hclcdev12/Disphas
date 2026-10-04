//! Tests on a real test drive. Never run by default or in CI (CLAUDE.md §2).
//!
//! Requirements, all of them:
//! - feature `hw-tests`, run as root on the test bench;
//! - `OBLIVIO_TEST_DEVICE`: kernel name of the dedicated test drive (`sdb`, `nvme1n1`);
//! - `OBLIVIO_TEST_SERIAL`: its serial number, read again from the firmware and compared;
//! - destructive tests are `#[ignore]` and also need
//!   `OBLIVIO_TEST_DESTRUCTIVE=I-ACCEPT-DATA-LOSS-ON-THE-TEST-DRIVE`.
//!
//! ```text
//! sudo -E env OBLIVIO_TEST_DEVICE=sdb OBLIVIO_TEST_SERIAL=… \
//!   cargo test -p oblivio-device --features hw-tests --test hw
//! ```
#![cfg(feature = "hw-tests")]
#![allow(clippy::expect_used)]

use oblivio_core::profile::Inspection;
use oblivio_device::block::BlockIo;
use oblivio_device::drive::ErasableDrive;
use oblivio_device::inspect::{Inspected, inspect};
use oblivio_device::{SystemPaths, scan};

fn test_drive() -> (SystemPaths, Inspected) {
    let name = std::env::var("OBLIVIO_TEST_DEVICE").expect("OBLIVIO_TEST_DEVICE is required");
    let serial = std::env::var("OBLIVIO_TEST_SERIAL").expect("OBLIVIO_TEST_SERIAL is required");
    let paths = SystemPaths::default();
    let drive = scan(&paths)
        .expect("inventory")
        .into_iter()
        .find(|drive| drive.name == name)
        .expect("test drive in the inventory");
    let inspected = inspect(&paths, &drive).expect("NVMe or SATA test drive");
    assert_eq!(
        inspected.profile.identity.serial, serial,
        "OBLIVIO_TEST_SERIAL does not match the firmware: wrong drive, stopping"
    );
    (paths, inspected)
}

#[test]
fn inspects_the_test_drive() {
    let (_, inspected) = test_drive();
    assert!(!matches!(inspected.profile.inspection, Inspection::Failed));
    assert!(inspected.capacity_bytes.is_some());
    println!("{inspected:#?}");
}

#[test]
#[ignore = "destructive: writes to the test drive"]
fn writes_and_reads_back_a_block() {
    assert_eq!(
        std::env::var("OBLIVIO_TEST_DESTRUCTIVE").as_deref(),
        Ok("I-ACCEPT-DATA-LOSS-ON-THE-TEST-DRIVE"),
        "destructive test not acknowledged"
    );
    let (paths, inspected) = test_drive();
    let mut drive = ErasableDrive::open(&paths, &inspected.name).expect("opened for erasure");
    let block = drive.block_size() as usize;
    let pattern: Vec<u8> = (0..block).map(|index| index as u8).collect();
    let last = drive.block_count() - 1;
    drive.write_blocks(last, &pattern).expect("write");
    drive.sync().expect("sync");
    let mut back = vec![0u8; block];
    drive.read_blocks(last, &mut back).expect("read");
    assert_eq!(back, pattern);
}
