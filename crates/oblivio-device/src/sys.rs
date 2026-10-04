//! Raw Linux kernel interface: the structures and ioctls Oblivio uses, and
//! nothing else. Every `unsafe` block of the project that talks to the
//! kernel is here, reduced to the system call itself.
//!
//! Layouts and numbers are those of the kernel UAPI headers
//! (`scsi/sg.h`, `linux/nvme_ioctl.h`, `linux/fs.h`) on x86_64; the tests
//! pin them to values read from the headers on 2026-10-04.

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

/// `struct nvme_passthru_cmd` (alias `nvme_admin_cmd`), 72 bytes.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct NvmePassthruCmd {
    pub opcode: u8,
    pub flags: u8,
    pub rsvd1: u16,
    pub nsid: u32,
    pub cdw2: u32,
    pub cdw3: u32,
    pub metadata: u64,
    pub addr: u64,
    pub metadata_len: u32,
    pub data_len: u32,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
    pub timeout_ms: u32,
    pub result: u32,
}

/// `struct sg_io_hdr`, 88 bytes on 64-bit Linux.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SgIoHdr {
    pub interface_id: i32,
    pub dxfer_direction: i32,
    pub cmd_len: u8,
    pub mx_sb_len: u8,
    pub iovec_count: u16,
    pub dxfer_len: u32,
    pub dxferp: *mut u8,
    pub cmdp: *const u8,
    pub sbp: *mut u8,
    pub timeout: u32,
    pub flags: u32,
    pub pack_id: i32,
    pub usr_ptr: *mut u8,
    pub status: u8,
    pub masked_status: u8,
    pub msg_status: u8,
    pub sb_len_wr: u8,
    pub host_status: u16,
    pub driver_status: u16,
    pub resid: i32,
    pub duration: u32,
    pub info: u32,
}

/// `NVME_IOCTL_ID`: namespace id of a namespace block device.
pub const NVME_IOCTL_ID: libc::c_ulong = 0x4e40;
/// `NVME_IOCTL_ADMIN_CMD` = `_IOWR('N', 0x41, struct nvme_admin_cmd)`.
pub const NVME_IOCTL_ADMIN_CMD: libc::c_ulong = 0xc048_4e41;
/// `SG_IO`.
pub const SG_IO: libc::c_ulong = 0x2285;
/// `BLKGETSIZE64` = `_IOR(0x12, 114, size_t)`: device size in bytes.
pub const BLKGETSIZE64: libc::c_ulong = 0x8008_1272;
/// `BLKSSZGET`: logical block size.
pub const BLKSSZGET: libc::c_ulong = 0x1268;

/// `SG_DXFER_NONE`.
pub const SG_DXFER_NONE: i32 = -1;
/// `SG_DXFER_FROM_DEV`.
pub const SG_DXFER_FROM_DEV: i32 = -3;

fn check(result: libc::c_int) -> io::Result<libc::c_int> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

/// Sends an NVMe admin command; returns the completion `result` (DW0).
/// A positive ioctl return value is the NVMe status of a failed command.
///
/// The caller guarantees that `cmd.addr` is 0 or points to a live buffer of
/// at least `cmd.data_len` bytes, valid for the duration of the call.
pub fn nvme_admin(fd: BorrowedFd<'_>, cmd: &mut NvmePassthruCmd) -> io::Result<u32> {
    // SAFETY: `fd` is an open descriptor; `cmd` is an exclusively borrowed,
    // properly laid out `nvme_admin_cmd` that the kernel reads and updates
    // (`result`); its data buffer is guaranteed live by the caller (above).
    let status = check(unsafe {
        libc::ioctl(
            fd.as_raw_fd(),
            NVME_IOCTL_ADMIN_CMD,
            cmd as *mut NvmePassthruCmd,
        )
    })?;
    if status != 0 {
        return Err(io::Error::other(format!("NVMe status 0x{status:x}")));
    }
    Ok(cmd.result)
}

/// Namespace id of an NVMe namespace block device.
pub fn nvme_namespace_id(fd: BorrowedFd<'_>) -> io::Result<u32> {
    // SAFETY: `NVME_IOCTL_ID` takes no argument and only returns a value.
    let nsid = check(unsafe { libc::ioctl(fd.as_raw_fd(), NVME_IOCTL_ID) })?;
    u32::try_from(nsid).map_err(|_| io::Error::other("invalid namespace id"))
}

/// Sends a SCSI command through SG_IO.
///
/// The caller guarantees that `hdr.cmdp` points to `hdr.cmd_len` bytes,
/// `hdr.sbp` to `hdr.mx_sb_len` bytes and `hdr.dxferp` to `hdr.dxfer_len`
/// bytes, all live for the duration of the call.
pub fn sg_io(fd: BorrowedFd<'_>, hdr: &mut SgIoHdr) -> io::Result<()> {
    // SAFETY: `fd` is an open descriptor; `hdr` is an exclusively borrowed
    // `sg_io_hdr` whose pointers the caller guarantees (above); the kernel
    // only accesses the memory they describe and fills the output fields.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), SG_IO, hdr as *mut SgIoHdr) })?;
    Ok(())
}

/// Size of a block device in bytes.
pub fn block_device_size(fd: BorrowedFd<'_>) -> io::Result<u64> {
    let mut size: u64 = 0;
    // SAFETY: `BLKGETSIZE64` writes one `u64` to the pointer, which points
    // to a live local variable.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), BLKGETSIZE64, &mut size as *mut u64) })?;
    Ok(size)
}

/// Logical block size of a block device.
pub fn logical_block_size(fd: BorrowedFd<'_>) -> io::Result<u32> {
    let mut size: libc::c_int = 0;
    // SAFETY: `BLKSSZGET` writes one `int` to the pointer, which points to a
    // live local variable.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), BLKSSZGET, &mut size as *mut libc::c_int) })?;
    u32::try_from(size).map_err(|_| io::Error::other("invalid block size"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// Values printed by a C program from the kernel UAPI headers.
    #[test]
    fn nvme_passthru_cmd_matches_the_kernel() {
        assert_eq!(size_of::<NvmePassthruCmd>(), 72);
        assert_eq!(offset_of!(NvmePassthruCmd, opcode), 0);
        assert_eq!(offset_of!(NvmePassthruCmd, nsid), 4);
        assert_eq!(offset_of!(NvmePassthruCmd, metadata), 16);
        assert_eq!(offset_of!(NvmePassthruCmd, addr), 24);
        assert_eq!(offset_of!(NvmePassthruCmd, metadata_len), 32);
        assert_eq!(offset_of!(NvmePassthruCmd, data_len), 36);
        assert_eq!(offset_of!(NvmePassthruCmd, cdw10), 40);
        assert_eq!(offset_of!(NvmePassthruCmd, cdw15), 60);
        assert_eq!(offset_of!(NvmePassthruCmd, timeout_ms), 64);
        assert_eq!(offset_of!(NvmePassthruCmd, result), 68);
    }

    #[test]
    fn sg_io_hdr_matches_the_kernel() {
        assert_eq!(size_of::<SgIoHdr>(), 88);
        let offsets = [
            (offset_of!(SgIoHdr, interface_id), 0),
            (offset_of!(SgIoHdr, dxfer_direction), 4),
            (offset_of!(SgIoHdr, cmd_len), 8),
            (offset_of!(SgIoHdr, mx_sb_len), 9),
            (offset_of!(SgIoHdr, iovec_count), 10),
            (offset_of!(SgIoHdr, dxfer_len), 12),
            (offset_of!(SgIoHdr, dxferp), 16),
            (offset_of!(SgIoHdr, cmdp), 24),
            (offset_of!(SgIoHdr, sbp), 32),
            (offset_of!(SgIoHdr, timeout), 40),
            (offset_of!(SgIoHdr, flags), 44),
            (offset_of!(SgIoHdr, pack_id), 48),
            (offset_of!(SgIoHdr, usr_ptr), 56),
            (offset_of!(SgIoHdr, status), 64),
            (offset_of!(SgIoHdr, masked_status), 65),
            (offset_of!(SgIoHdr, msg_status), 66),
            (offset_of!(SgIoHdr, sb_len_wr), 67),
            (offset_of!(SgIoHdr, host_status), 68),
            (offset_of!(SgIoHdr, driver_status), 70),
            (offset_of!(SgIoHdr, resid), 72),
            (offset_of!(SgIoHdr, duration), 76),
            (offset_of!(SgIoHdr, info), 80),
        ];
        for (actual, expected) in offsets {
            assert_eq!(actual, expected);
        }
    }

    /// `_IOC(dir, type, nr, size)` = dir << 30 | size << 16 | type << 8 | nr.
    #[test]
    fn ioctl_numbers_match_the_kernel() {
        let ioc = |dir: u64, kind: u8, nr: u8, size: usize| {
            (dir << 30 | (size as u64) << 16 | u64::from(kind) << 8 | u64::from(nr))
                as libc::c_ulong
        };
        assert_eq!(
            NVME_IOCTL_ADMIN_CMD,
            ioc(3, b'N', 0x41, size_of::<NvmePassthruCmd>())
        );
        assert_eq!(NVME_IOCTL_ID, ioc(0, b'N', 0x40, 0));
        assert_eq!(BLKGETSIZE64, ioc(2, 0x12, 114, size_of::<u64>()));
        assert_eq!(SG_IO, 0x2285);
        assert_eq!(BLKSSZGET, 0x1268);
    }
}
