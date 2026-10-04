//! Low-level access to storage devices (NVMe and ATA ioctls, SG_IO,
//! O_DIRECT with aligned buffers, sysfs).
//!
//! This is the only crate allowed to talk to a device. It must refuse a
//! PROTECTED drive by construction, including in tests. Hardware tests live
//! behind the `hw-tests` feature and the `OBLIVIO_TEST_DEVICE` variable.
//!
//! So far it holds the read-only [`inventory`] (sysfs and procfs, no device
//! node opened, no ioctl sent) and pure [`decode`]rs for firmware data.

pub mod block;
pub mod decode;
pub mod inventory;
pub mod procfs;
pub mod sysfs;

pub use inventory::{BlockDevice, ScanError, SystemPaths, scan};
