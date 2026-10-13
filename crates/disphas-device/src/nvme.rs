//! NVMe admin commands (NVM Express Base Specification 2.0): Identify, Get
//! Log Page, Sanitize and Format NVM.
//!
//! Building a command is pure and tested; sending it goes through
//! `sys::nvme_admin`. Never exercised on a real drive outside `hw-tests`.

use std::fs::File;
use std::os::fd::AsFd;

use disphas_core::profile::{NvmeHealth, SanitizeState};

use crate::DeviceError;
use crate::aligned::AlignedBuffer;
use crate::decode::{self, nvme as decoded};
use crate::sys::{self, NvmePassthruCmd};

/// Admin opcodes.
const GET_LOG_PAGE: u8 = 0x02;
const IDENTIFY: u8 = 0x06;
const FORMAT_NVM: u8 = 0x80;
const SANITIZE: u8 = 0x84;

/// Log pages.
const LOG_SMART_HEALTH: u8 = 0x02;
const LOG_SANITIZE_STATUS: u8 = 0x81;

/// Every namespace (`NSID FFFFFFFFh`).
const ALL_NAMESPACES: u32 = 0xffff_ffff;

/// Timeouts.
const SHORT_TIMEOUT_MS: u32 = 10_000;
const SANITIZE_START_TIMEOUT_MS: u32 = 60_000;
/// Format NVM blocks until done: a user-data erase can take hours.
const FORMAT_TIMEOUT_MS: u32 = 4 * 3600 * 1000;

/// A built admin command, before its data buffer is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminCommand {
    /// Opcode.
    pub opcode: u8,
    /// Namespace id.
    pub nsid: u32,
    /// Command dword 10.
    pub cdw10: u32,
    /// Command dword 11.
    pub cdw11: u32,
    /// Bytes transferred from the controller (0 for none).
    pub data_len: u32,
    /// Timeout in milliseconds.
    pub timeout_ms: u32,
}

/// Identify with a given CNS (00h namespace, 01h controller), 4096 bytes.
pub fn identify(cns: u8, nsid: u32) -> AdminCommand {
    AdminCommand {
        opcode: IDENTIFY,
        nsid,
        cdw10: u32::from(cns),
        cdw11: 0,
        data_len: decoded::IDENTIFY_SIZE as u32,
        timeout_ms: SHORT_TIMEOUT_MS,
    }
}

/// Get Log Page of `bytes` bytes (a multiple of 4): NUMD = dwords - 1,
/// low half in CDW10 bits 31:16, high half in CDW11 bits 15:0.
pub fn get_log_page(lid: u8, nsid: u32, bytes: u32) -> AdminCommand {
    let numd = bytes / 4 - 1;
    AdminCommand {
        opcode: GET_LOG_PAGE,
        nsid,
        cdw10: u32::from(lid) | (numd & 0xffff) << 16,
        cdw11: numd >> 16,
        data_len: bytes,
        timeout_ms: SHORT_TIMEOUT_MS,
    }
}

/// Sanitize actions (CDW10 SANACT, bits 2:0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizeAction {
    /// 001b: exit failure mode.
    ExitFailureMode,
    /// 010b: block erase.
    BlockErase,
    /// 100b: crypto erase.
    CryptoErase,
}

/// Sanitize command. Applies to the whole controller (NSID 0). AUSE is
/// cleared: a failure is not recovered silently, it is reported (B5).
pub fn sanitize(action: SanitizeAction) -> AdminCommand {
    let sanact = match action {
        SanitizeAction::ExitFailureMode => 0b001,
        SanitizeAction::BlockErase => 0b010,
        SanitizeAction::CryptoErase => 0b100,
    };
    AdminCommand {
        opcode: SANITIZE,
        nsid: 0,
        cdw10: sanact,
        cdw11: 0,
        data_len: 0,
        timeout_ms: SANITIZE_START_TIMEOUT_MS,
    }
}

/// Secure erase setting of Format NVM (CDW10 SES, bits 11:9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureErase {
    /// 001b: user data erase.
    UserData,
    /// 010b: cryptographic erase.
    Cryptographic,
}

/// Format NVM keeping the namespace's current LBA format `lbaf` (0..63,
/// split between CDW10 bits 3:0 and 13:12), no metadata or protection change.
pub fn format(nsid: u32, lbaf: u8, ses: SecureErase) -> AdminCommand {
    let ses = match ses {
        SecureErase::UserData => 0b001,
        SecureErase::Cryptographic => 0b010,
    };
    let lbaf = u32::from(lbaf);
    AdminCommand {
        opcode: FORMAT_NVM,
        nsid,
        cdw10: (lbaf & 0x0f) | ses << 9 | (lbaf >> 4 & 0x03) << 12,
        cdw11: 0,
        data_len: 0,
        timeout_ms: FORMAT_TIMEOUT_MS,
    }
}

/// Current LBA format index of an Identify Namespace structure (FLBAS).
pub fn current_lba_format(identify_namespace: &[u8]) -> Option<u8> {
    let flbas = *identify_namespace.get(26)?;
    Some((flbas & 0x0f) | ((flbas >> 5) & 0x03) << 4)
}

/// Progress of the running sanitize, SPROG (bytes 0..2) out of 65536.
pub fn sanitize_progress(log: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes([*log.first()?, *log.get(1)?]))
}

/// An NVMe namespace block device opened by disphas.
pub struct NvmeDevice<'a> {
    file: &'a File,
    nsid: u32,
}

impl<'a> NvmeDevice<'a> {
    /// The namespace behind `file`, an opened `/dev/nvmeXnY`.
    pub fn new(file: &'a File) -> Result<Self, DeviceError> {
        let nsid = sys::nvme_namespace_id(file.as_fd())?;
        Ok(Self { file, nsid })
    }

    /// Namespace id.
    pub fn nsid(&self) -> u32 {
        self.nsid
    }

    fn run(&self, command: AdminCommand) -> Result<AlignedBuffer, DeviceError> {
        let mut data = AlignedBuffer::new(command.data_len as usize);
        let mut raw = NvmePassthruCmd {
            opcode: command.opcode,
            nsid: command.nsid,
            cdw10: command.cdw10,
            cdw11: command.cdw11,
            data_len: command.data_len,
            addr: if command.data_len == 0 {
                0
            } else {
                data.as_mut_ptr() as u64
            },
            timeout_ms: command.timeout_ms,
            ..NvmePassthruCmd::default()
        };
        // `data` outlives the call and holds `data_len` bytes.
        sys::nvme_admin(self.file.as_fd(), &mut raw)?;
        Ok(data)
    }

    /// Identify Controller, decoded.
    pub fn identify_controller(&self) -> Result<decoded::IdentifyController, DeviceError> {
        Ok(decoded::identify_controller(&self.run(identify(0x01, 0))?)?)
    }

    /// Identify Namespace, raw (for the current LBA format) and decoded.
    pub fn identify_namespace(
        &self,
    ) -> Result<(AlignedBuffer, decoded::IdentifyNamespace), DeviceError> {
        let data = self.run(identify(0x00, self.nsid))?;
        let namespace = decoded::identify_namespace(&data)?;
        Ok((data, namespace))
    }

    /// SMART / Health Information log.
    pub fn health(&self) -> Result<NvmeHealth, DeviceError> {
        let log = self.run(get_log_page(
            LOG_SMART_HEALTH,
            ALL_NAMESPACES,
            decoded::HEALTH_LOG_SIZE as u32,
        ))?;
        Ok(decoded::health(&log)?)
    }

    /// Sanitize Status log: state and progress (out of 65536).
    pub fn sanitize_status(&self) -> Result<(SanitizeState, u16), DeviceError> {
        let log = self.run(get_log_page(LOG_SANITIZE_STATUS, 0, 512))?;
        let state = decoded::sanitize_state(&log)?;
        let progress = sanitize_progress(&log).ok_or(decode::DecodeError::TooShort {
            structure: "NVMe Sanitize Status log",
            expected: 2,
            actual: log.len(),
        })?;
        Ok((state, progress))
    }

    /// Starts a sanitize operation; it runs in the background.
    pub fn start_sanitize(&self, action: SanitizeAction) -> Result<(), DeviceError> {
        self.run(sanitize(action)).map(|_| ())
    }

    /// Format NVM with a secure erase, keeping the current LBA format.
    /// Blocks until the controller completes it.
    pub fn format(&self, ses: SecureErase) -> Result<(), DeviceError> {
        let (raw, _) = self.identify_namespace()?;
        let lbaf = current_lba_format(&raw).ok_or(DeviceError::Command(
            "cannot read the current LBA format".to_owned(),
        ))?;
        self.run(format(self.nsid, lbaf, ses)).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_commands() {
        let controller = identify(0x01, 0);
        assert_eq!(
            (controller.opcode, controller.nsid, controller.cdw10),
            (0x06, 0, 1)
        );
        assert_eq!(controller.data_len, 4096);
        assert_eq!(identify(0x00, 3).nsid, 3);
    }

    #[test]
    fn get_log_page_encodes_dword_count() {
        // 512 bytes = 128 dwords → NUMD 127 (0x7f).
        let smart = get_log_page(0x02, 0xffff_ffff, 512);
        assert_eq!(smart.cdw10, 0x007f_0002);
        assert_eq!(smart.cdw11, 0);
        // 512 KiB = 131072 dwords → NUMD 131071: low half 0xffff, high half 1.
        let big = get_log_page(0x81, 0, 512 * 1024);
        assert_eq!(big.cdw10, 0xffff_0081);
        assert_eq!(big.cdw11, 1);
    }

    #[test]
    fn sanitize_actions() {
        assert_eq!(sanitize(SanitizeAction::BlockErase).cdw10, 0b010);
        assert_eq!(sanitize(SanitizeAction::CryptoErase).cdw10, 0b100);
        assert_eq!(sanitize(SanitizeAction::ExitFailureMode).cdw10, 0b001);
        let command = sanitize(SanitizeAction::BlockErase);
        assert_eq!(
            (command.opcode, command.nsid, command.data_len),
            (0x84, 0, 0)
        );
    }

    #[test]
    fn format_keeps_the_lba_format_and_sets_ses() {
        // LBA format 1, cryptographic erase: SES 010b in bits 11:9.
        assert_eq!(format(1, 1, SecureErase::Cryptographic).cdw10, 0x0401);
        assert_eq!(format(1, 0, SecureErase::UserData).cdw10, 0x0200);
        // LBA format 17 = 0b01_0001: low nibble 1, upper bits 01 in 13:12.
        assert_eq!(format(1, 17, SecureErase::UserData).cdw10, 0x1201);
    }

    #[test]
    fn reads_current_lba_format() {
        let mut identify = vec![0u8; 4096];
        identify[26] = 0b0010_0011; // bits 6:5 = 01, bits 3:0 = 3 → 19
        assert_eq!(current_lba_format(&identify), Some(19));
    }

    #[test]
    fn reads_sanitize_progress() {
        assert_eq!(sanitize_progress(&[0x00, 0x80, 2, 0]), Some(0x8000));
    }
}
