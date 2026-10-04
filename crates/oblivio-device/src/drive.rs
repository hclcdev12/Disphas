//! A real drive opened for erasure.
//!
//! Opening refuses a protected drive by construction (CLAUDE.md §2): a
//! fresh inventory is taken, a drive with any protection (P1 to P5) is
//! refused, only block devices are accepted, and the node is opened with
//! `O_EXCL`, so the kernel itself refuses if the drive is mounted or held.
//! Reads and writes use `O_DIRECT` with page-aligned buffers.

use std::fs::{File, OpenOptions};
use std::os::fd::AsFd;
use std::os::unix::fs::{FileExt, FileTypeExt, OpenOptionsExt};

use crate::aligned::AlignedBuffer;
use crate::block::BlockIo;
use crate::inventory::{self, SystemPaths, Transport};
use crate::{DeviceError, sys};

/// How the drive is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interface {
    /// NVMe namespace.
    Nvme,
    /// SATA through SG_IO.
    Ata,
}

/// A drive opened read-write for erasure.
pub struct ErasableDrive {
    file: File,
    name: String,
    interface: Interface,
    block_size: u64,
    block_count: u64,
}

impl ErasableDrive {
    /// Opens the drive named `name` (`nvme0n1`, `sda`) for erasure, after a
    /// fresh inventory. Refuses a drive that is protected, absent, not NVMe
    /// or SATA, not a block device, or that the kernel reports as in use.
    pub fn open(paths: &SystemPaths, name: &str) -> Result<Self, DeviceError> {
        let drives = inventory::scan(paths)?;
        let drive = drives
            .iter()
            .find(|drive| drive.name == name)
            .ok_or_else(|| DeviceError::Refused(format!("{name} is not in the inventory")))?;
        let facts = drive.protection;
        if facts.hosts_system
            || facts.has_mounted_partition
            || facts.has_active_swap
            || facts.is_volume_member
            || facts.sibling_namespace_protected
        {
            return Err(DeviceError::Refused(format!("{name} is protected")));
        }
        let interface = match drive.transport {
            Transport::Nvme => Interface::Nvme,
            Transport::Ata => Interface::Ata,
            Transport::Usb | Transport::Other => {
                return Err(DeviceError::Refused(format!(
                    "{name}: transport not supported in V1"
                )));
            }
        };
        let path = paths.dev.join(name);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_block_device() {
            return Err(DeviceError::Refused(format!(
                "{} is not a block device",
                path.display()
            )));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_DIRECT | libc::O_EXCL)
            .open(&path)
            .map_err(|error| match error.raw_os_error() {
                Some(libc::EBUSY) => DeviceError::Refused(format!("{name} is in use (O_EXCL)")),
                _ => DeviceError::Io(error),
            })?;
        let block_size = u64::from(sys::logical_block_size(file.as_fd())?);
        let size = sys::block_device_size(file.as_fd())?;
        if block_size == 0 || size % block_size != 0 {
            return Err(DeviceError::Command(
                "inconsistent drive geometry".to_owned(),
            ));
        }
        Ok(Self {
            file,
            name: name.to_owned(),
            interface,
            block_size,
            block_count: size / block_size,
        })
    }

    /// Kernel name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// NVMe or SATA.
    pub fn interface(&self) -> Interface {
        self.interface
    }

    /// The opened node, for firmware commands.
    pub fn file(&self) -> &File {
        &self.file
    }

    fn check(&self, lba: u64, len: usize) -> std::io::Result<u64> {
        let len = len as u64;
        let fits = len % self.block_size == 0
            && lba
                .checked_add(len / self.block_size)
                .is_some_and(|end| end <= self.block_count);
        if !fits {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "access outside the drive or not block-aligned",
            ));
        }
        Ok(lba * self.block_size)
    }
}

impl BlockIo for ErasableDrive {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn block_count(&self) -> u64 {
        self.block_count
    }

    fn read_blocks(&mut self, lba: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        let offset = self.check(lba, buffer.len())?;
        // O_DIRECT needs an aligned buffer: read into one, then copy.
        let mut aligned = AlignedBuffer::new(buffer.len());
        self.file.read_exact_at(&mut aligned, offset)?;
        buffer.copy_from_slice(&aligned);
        Ok(())
    }

    fn write_blocks(&mut self, lba: u64, data: &[u8]) -> std::io::Result<()> {
        let offset = self.check(lba, data.len())?;
        let mut aligned = AlignedBuffer::new(data.len());
        aligned.copy_from_slice(data);
        self.file.write_all_at(&aligned, offset)
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.file.sync_all()
    }
}
