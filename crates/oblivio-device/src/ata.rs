//! ATA commands through SG_IO and ATA PASS-THROUGH (16) (SAT-4, ACS-4):
//! IDENTIFY DEVICE, SMART, SANITIZE.
//!
//! Building a command block and reading the returned registers are pure and
//! tested; sending goes through `sys::sg_io`. Never exercised on a real
//! drive outside `hw-tests`.

use std::fs::File;
use std::os::fd::AsFd;

use crate::DeviceError;
use crate::aligned::AlignedBuffer;
use crate::decode::ata as decoded;
use crate::sys::{self, SgIoHdr};

/// ATA PASS-THROUGH (16) operation code.
const PASS_THROUGH_16: u8 = 0x85;

/// ATA commands.
const IDENTIFY_DEVICE: u8 = 0xec;
const SMART: u8 = 0xb0;
const SANITIZE_DEVICE: u8 = 0xb4;

/// SMART features and signature (LBA Mid 4Fh, LBA High C2h).
const SMART_READ_DATA: u16 = 0xd0;
const SMART_RETURN_STATUS: u16 = 0xda;
const SMART_SIGNATURE: u64 = 0xc2_4f00;

/// SANITIZE DEVICE features and their LBA signatures.
const SANITIZE_STATUS_EXT: u16 = 0x0000;
const CRYPTO_SCRAMBLE_EXT: u16 = 0x0011;
const BLOCK_ERASE_EXT: u16 = 0x0012;
const OVERWRITE_EXT: u16 = 0x0014;
const CRYPTO_SIGNATURE: u64 = 0x4372_7970; // "Cryp"
const BLOCK_ERASE_SIGNATURE: u64 = 0x426b_4572; // "BkEr"
const OVERWRITE_SIGNATURE: u64 = 0x4f57 << 32; // "OW" in LBA 47:32

const SHORT_TIMEOUT_MS: u32 = 10_000;
const SANITIZE_START_TIMEOUT_MS: u32 = 60_000;

/// Data transfer of an ATA command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transfer {
    /// No data; registers returned (CK_COND).
    None,
    /// PIO data-in of `n` 512-byte sectors.
    In(u16),
}

/// An ATA command to pass through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtaCommand {
    /// Command code.
    pub command: u8,
    /// Features.
    pub features: u16,
    /// Count.
    pub count: u16,
    /// LBA (48 bits).
    pub lba: u64,
    /// Device register.
    pub device: u8,
    /// 48-bit (EXT) command.
    pub extend: bool,
    /// Data transfer.
    pub transfer: Transfer,
    /// Timeout in milliseconds.
    pub timeout_ms: u32,
}

/// The ATA PASS-THROUGH (16) command block of `command` (SAT-4 §12.2.2).
pub fn cdb(command: &AtaCommand) -> [u8; 16] {
    let (protocol, flags) = match command.transfer {
        // Non-data; CK_COND: return the registers in the sense data.
        Transfer::None => (3u8, 0x20u8),
        // PIO data-in; T_DIR from device, BYTE_BLOCK, T_LENGTH in COUNT.
        Transfer::In(_) => (4, 0x0e),
    };
    let lba = command.lba.to_be_bytes(); // lba[2..8] = bits 47..0
    let [features_high, features_low] = command.features.to_be_bytes();
    let [count_high, count_low] = command.count.to_be_bytes();
    [
        PASS_THROUGH_16,
        protocol << 1 | u8::from(command.extend),
        flags,
        features_high,
        features_low,
        count_high,
        count_low,
        lba[4], // LBA 31:24
        lba[7], // LBA 7:0
        lba[3], // LBA 39:32
        lba[6], // LBA 15:8
        lba[2], // LBA 47:40
        lba[5], // LBA 23:16
        command.device,
        command.command,
        0,
    ]
}

/// IDENTIFY DEVICE: one sector in.
pub fn identify_device() -> AtaCommand {
    AtaCommand {
        command: IDENTIFY_DEVICE,
        features: 0,
        count: 1,
        lba: 0,
        device: 0,
        extend: false,
        transfer: Transfer::In(1),
        timeout_ms: SHORT_TIMEOUT_MS,
    }
}

/// SMART READ DATA: one sector in.
pub fn smart_read_data() -> AtaCommand {
    AtaCommand {
        command: SMART,
        features: SMART_READ_DATA,
        count: 1,
        lba: SMART_SIGNATURE,
        device: 0,
        extend: false,
        transfer: Transfer::In(1),
        timeout_ms: SHORT_TIMEOUT_MS,
    }
}

/// SMART RETURN STATUS: the verdict comes back in LBA Mid and LBA High.
pub fn smart_return_status() -> AtaCommand {
    AtaCommand {
        command: SMART,
        features: SMART_RETURN_STATUS,
        count: 0,
        lba: SMART_SIGNATURE,
        device: 0,
        extend: false,
        transfer: Transfer::None,
        timeout_ms: SHORT_TIMEOUT_MS,
    }
}

/// SANITIZE STATUS EXT; `clear_failure` sets CLEAR SANITIZE OPERATION FAILED
/// (COUNT bit 0) to leave Failure mode (B5).
pub fn sanitize_status_ext(clear_failure: bool) -> AtaCommand {
    sanitize_device(
        SANITIZE_STATUS_EXT,
        u16::from(clear_failure),
        0,
        SHORT_TIMEOUT_MS,
    )
}

/// Sanitize operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizeOperation {
    /// BLOCK ERASE EXT.
    BlockErase,
    /// CRYPTO SCRAMBLE EXT.
    CryptoScramble,
    /// OVERWRITE EXT: one pass of zeros, no inversion.
    Overwrite,
}

/// The SANITIZE DEVICE command starting `operation`. FAILURE MODE is
/// cleared: a failed operation leaves the drive in Failure mode, reported.
pub fn sanitize(operation: SanitizeOperation) -> AtaCommand {
    match operation {
        SanitizeOperation::BlockErase => sanitize_device(
            BLOCK_ERASE_EXT,
            0,
            BLOCK_ERASE_SIGNATURE,
            SANITIZE_START_TIMEOUT_MS,
        ),
        SanitizeOperation::CryptoScramble => sanitize_device(
            CRYPTO_SCRAMBLE_EXT,
            0,
            CRYPTO_SIGNATURE,
            SANITIZE_START_TIMEOUT_MS,
        ),
        // OVERWRITE COUNT 1 (COUNT bits 3:0); pattern 00000000h in LBA 31:0.
        SanitizeOperation::Overwrite => sanitize_device(
            OVERWRITE_EXT,
            1,
            OVERWRITE_SIGNATURE,
            SANITIZE_START_TIMEOUT_MS,
        ),
    }
}

fn sanitize_device(features: u16, count: u16, lba: u64, timeout_ms: u32) -> AtaCommand {
    AtaCommand {
        command: SANITIZE_DEVICE,
        features,
        count,
        lba,
        device: 0x40,
        extend: true,
        transfer: Transfer::None,
        timeout_ms,
    }
}

/// ATA registers returned by the device (ATA Status Return descriptor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    /// Error register.
    pub error: u8,
    /// Count.
    pub count: u16,
    /// LBA.
    pub lba: u64,
    /// Device register.
    pub device: u8,
    /// Status register; bit 0 is ERR.
    pub status: u8,
}

impl Registers {
    /// The command failed (ERR set in Status).
    pub fn failed(&self) -> bool {
        self.status & 0x01 != 0
    }

    /// LBA Mid (LBA 15:8) and LBA High (LBA 23:16).
    pub fn lba_mid_high(&self) -> (u8, u8) {
        ((self.lba >> 8) as u8, (self.lba >> 16) as u8)
    }
}

/// The ATA Status Return descriptor (code 09h) of descriptor-format sense
/// data (response code 72h or 73h), SAT-4 §12.2.2.6.
pub fn registers_from_sense(sense: &[u8]) -> Option<Registers> {
    let response = *sense.first()? & 0x7f;
    if response != 0x72 && response != 0x73 {
        return None;
    }
    let total = 8 + usize::from(*sense.get(7)?);
    let mut offset = 8;
    while offset + 2 <= total.min(sense.len()) {
        let code = sense[offset];
        let length = usize::from(sense[offset + 1]);
        if code == 0x09 && length >= 0x0c {
            let d = sense.get(offset..offset + 14)?;
            let byte = |index: usize| u64::from(d[index]);
            let extend = d[2] & 0x01 != 0;
            let lba = byte(7)
                | byte(9) << 8
                | byte(11) << 16
                | if extend {
                    byte(6) << 24 | byte(8) << 32 | byte(10) << 40
                } else {
                    0
                };
            return Some(Registers {
                error: d[3],
                count: u16::from(d[5]) | if extend { u16::from(d[4]) << 8 } else { 0 },
                lba,
                device: d[12],
                status: d[13],
            });
        }
        offset += 2 + length;
    }
    None
}

/// A SATA drive opened by Oblivio (`/dev/sdX`).
pub struct AtaDevice<'a> {
    file: &'a File,
}

/// Outcome of a passed-through command.
struct Completion {
    data: AlignedBuffer,
    registers: Option<Registers>,
}

impl<'a> AtaDevice<'a> {
    /// The drive behind `file`.
    pub fn new(file: &'a File) -> Self {
        Self { file }
    }

    fn run(&self, command: &AtaCommand) -> Result<Completion, DeviceError> {
        let block = cdb(command);
        let mut sense = [0u8; 64];
        let sectors = match command.transfer {
            Transfer::None => 0,
            Transfer::In(sectors) => usize::from(sectors),
        };
        let mut data = AlignedBuffer::new(sectors * decoded::SECTOR_SIZE);
        let mut hdr = SgIoHdr {
            interface_id: i32::from(b'S'),
            dxfer_direction: if sectors == 0 {
                sys::SG_DXFER_NONE
            } else {
                sys::SG_DXFER_FROM_DEV
            },
            cmd_len: block.len() as u8,
            mx_sb_len: sense.len() as u8,
            iovec_count: 0,
            dxfer_len: (sectors * decoded::SECTOR_SIZE) as u32,
            dxferp: if sectors == 0 {
                std::ptr::null_mut()
            } else {
                data.as_mut_ptr()
            },
            cmdp: block.as_ptr(),
            sbp: sense.as_mut_ptr(),
            timeout: command.timeout_ms,
            flags: 0,
            pack_id: 0,
            usr_ptr: std::ptr::null_mut(),
            status: 0,
            masked_status: 0,
            msg_status: 0,
            sb_len_wr: 0,
            host_status: 0,
            driver_status: 0,
            resid: 0,
            duration: 0,
            info: 0,
        };
        // `block`, `sense` and `data` outlive the call and match the lengths.
        sys::sg_io(self.file.as_fd(), &mut hdr)?;
        if hdr.host_status != 0 || hdr.driver_status & !0x08 != 0 {
            return Err(DeviceError::Command(format!(
                "SG_IO host 0x{:x}, driver 0x{:x}",
                hdr.host_status, hdr.driver_status
            )));
        }
        let registers = registers_from_sense(&sense[..usize::from(hdr.sb_len_wr)]);
        if let Some(registers) = registers
            && registers.failed()
        {
            return Err(DeviceError::Command(format!(
                "ATA command 0x{:02x} aborted (error 0x{:02x})",
                command.command, registers.error
            )));
        }
        // CHECK CONDITION without ATA registers is a SCSI-level failure.
        if hdr.status != 0 && registers.is_none() {
            return Err(DeviceError::Command(format!(
                "SCSI status 0x{:x}",
                hdr.status
            )));
        }
        Ok(Completion { data, registers })
    }

    /// IDENTIFY DEVICE, decoded.
    pub fn identify(&self) -> Result<decoded::IdentifyDevice, DeviceError> {
        Ok(decoded::identify_device(
            &self.run(&identify_device())?.data,
        )?)
    }

    /// SMART status (failing) and attributes.
    pub fn smart(&self) -> Result<(bool, decoded::SmartAttributes), DeviceError> {
        let status = self
            .run(&smart_return_status())?
            .registers
            .ok_or_else(|| DeviceError::Command("SMART status without registers".to_owned()))?;
        let (mid, high) = status.lba_mid_high();
        let failing = decoded::smart_failing(mid, high)?;
        let attributes = decoded::smart_attributes(&self.run(&smart_read_data())?.data)?;
        Ok((failing, attributes))
    }

    /// SANITIZE STATUS EXT: state, freeze lock, and progress (LBA 15:0).
    pub fn sanitize_status(&self) -> Result<(decoded::SanitizeStatus, u16), DeviceError> {
        match self.run(&sanitize_status_ext(false)) {
            Ok(completion) => {
                let registers = completion.registers.ok_or_else(|| {
                    DeviceError::Command("sanitize status without registers".to_owned())
                })?;
                Ok((
                    decoded::sanitize_status(registers.count, false),
                    registers.lba as u16,
                ))
            }
            // An aborted SANITIZE STATUS EXT reports a failed operation.
            Err(DeviceError::Command(_)) => Ok((decoded::sanitize_status(0, true), 0)),
            Err(error) => Err(error),
        }
    }

    /// Starts a sanitize operation; it runs in the background.
    pub fn start_sanitize(&self, operation: SanitizeOperation) -> Result<(), DeviceError> {
        self.run(&sanitize(operation)).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_device_block() {
        let block = cdb(&identify_device());
        assert_eq!(
            block,
            [0x85, 0x08, 0x0e, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0xec, 0]
        );
    }

    #[test]
    fn smart_blocks_carry_the_signature() {
        let read = cdb(&smart_read_data());
        assert_eq!(read[4], 0xd0, "features");
        assert_eq!((read[10], read[12]), (0x4f, 0xc2), "LBA mid, LBA high");
        assert_eq!(read[14], 0xb0);
        let status = cdb(&smart_return_status());
        assert_eq!((status[1], status[2]), (0x06, 0x20), "non-data, CK_COND");
        assert_eq!(status[4], 0xda);
    }

    #[test]
    fn sanitize_blocks_carry_their_signatures() {
        let block_erase = cdb(&sanitize(SanitizeOperation::BlockErase));
        assert_eq!(block_erase[1], 0x07, "non-data, EXTEND");
        assert_eq!(block_erase[4], 0x12);
        // "BkEr" = 42 6B 45 72 over LBA 31:0.
        assert_eq!(
            [
                block_erase[7],
                block_erase[12],
                block_erase[10],
                block_erase[8]
            ],
            [0x42, 0x6b, 0x45, 0x72]
        );
        assert_eq!(block_erase[14], 0xb4);

        let crypto = cdb(&sanitize(SanitizeOperation::CryptoScramble));
        assert_eq!(crypto[4], 0x11);
        assert_eq!([crypto[7], crypto[12], crypto[10], crypto[8]], *b"Cryp");

        let overwrite = cdb(&sanitize(SanitizeOperation::Overwrite));
        assert_eq!(overwrite[4], 0x14);
        assert_eq!([overwrite[11], overwrite[9]], *b"OW", "LBA 47:32");
        assert_eq!(
            [overwrite[7], overwrite[12], overwrite[10], overwrite[8]],
            [0; 4],
            "pattern"
        );
        assert_eq!(overwrite[6], 1, "one pass");

        let status = cdb(&sanitize_status_ext(true));
        assert_eq!(
            (status[4], status[6]),
            (0x00, 0x01),
            "status, clear failure"
        );
    }

    fn sense_with(descriptor: [u8; 14]) -> Vec<u8> {
        let mut sense = vec![0x72, 0x01, 0x00, 0x1d, 0, 0, 0, 14];
        sense.extend_from_slice(&descriptor);
        sense
    }

    #[test]
    fn reads_smart_status_registers() {
        // 28-bit: LBA mid 4Fh, LBA high C2h, status 50h (ready, no error).
        let sense = sense_with([
            0x09, 0x0c, 0x00, 0x00, 0, 0, 0, 0, 0, 0x4f, 0, 0xc2, 0, 0x50,
        ]);
        let registers = registers_from_sense(&sense).expect("ATA descriptor");
        assert_eq!(registers.lba_mid_high(), (0x4f, 0xc2));
        assert!(!registers.failed());
        let failing = sense_with([
            0x09, 0x0c, 0x00, 0x00, 0, 0, 0, 0, 0, 0xf4, 0, 0x2c, 0, 0x50,
        ]);
        let (mid, high) = registers_from_sense(&failing)
            .expect("descriptor")
            .lba_mid_high();
        assert_eq!(decoded::smart_failing(mid, high), Ok(true));
    }

    #[test]
    fn reads_extended_count_and_lba() {
        // EXTEND: count 4000h (in progress), progress 8000h in LBA 15:0.
        let sense = sense_with([
            0x09, 0x0c, 0x01, 0x00, 0x40, 0x00, 0, 0x00, 0, 0x80, 0, 0, 0x40, 0x50,
        ]);
        let registers = registers_from_sense(&sense).expect("descriptor");
        assert_eq!(registers.count, 0x4000);
        assert_eq!(registers.lba as u16, 0x8000);
        assert!(
            decoded::sanitize_status(registers.count, false).state
                == oblivio_core::profile::SanitizeState::InProgress
        );
    }

    #[test]
    fn error_bit_marks_failure_and_fixed_sense_is_ignored() {
        let aborted = sense_with([0x09, 0x0c, 0x01, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0x40, 0x51]);
        assert!(registers_from_sense(&aborted).expect("descriptor").failed());
        assert_eq!(registers_from_sense(&[0x70, 0, 5, 0, 0, 0, 0, 10]), None);
        assert_eq!(registers_from_sense(&[]), None);
    }
}
