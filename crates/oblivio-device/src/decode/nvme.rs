//! NVMe data structures (NVM Express Base Specification 2.0).
//!
//! Offsets are in bytes, little-endian. To be cross-checked against
//! `smartctl --json -x` on the test bench (`docs/cadre_projet.md` §9).

use oblivio_core::profile::{
    CriticalWarning, NvmeFormatCaps, NvmeHealth, NvmeProfile, NvmeSanitizeCaps, SanitizeState,
};

use super::{DecodeError, ascii, hex_id, le_u16, le_u32, le_u64, le_u128, require};

/// Size of every Identify data structure.
pub const IDENTIFY_SIZE: usize = 4096;
/// Size of the SMART / Health Information log page (02h).
pub const HEALTH_LOG_SIZE: usize = 512;
/// Size of the Sanitize Status log page (81h) read by Oblivio.
pub const SANITIZE_LOG_SIZE: usize = 20;

/// Identify Controller (CNS 01h), fields used by Oblivio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifyController {
    /// SN, bytes 4..24.
    pub serial: String,
    /// MN, bytes 24..64.
    pub model: String,
    /// FR, bytes 64..72.
    pub firmware: String,
    /// SANICAP, bytes 328..332.
    pub sanitize: NvmeSanitizeCaps,
    /// OACS bit 1 and FNA (byte 524) bit 2.
    pub format: NvmeFormatCaps,
    /// FNA bit 1: a secure erase by Format NVM applies to every namespace.
    pub format_erases_all_namespaces: bool,
}

/// Decodes Identify Controller.
pub fn identify_controller(data: &[u8]) -> Result<IdentifyController, DecodeError> {
    require(data, IDENTIFY_SIZE, "NVMe Identify Controller")?;
    let oacs = le_u16(data, 256);
    let sanicap = le_u32(data, 328);
    let fna = data[524];
    Ok(IdentifyController {
        serial: ascii(&data[4..24]),
        model: ascii(&data[24..64]),
        firmware: ascii(&data[64..72]),
        sanitize: NvmeSanitizeCaps {
            crypto_erase: sanicap & (1 << 0) != 0,
            block_erase: sanicap & (1 << 1) != 0,
        },
        format: NvmeFormatCaps {
            supported: oacs & (1 << 1) != 0,
            crypto_erase: fna & (1 << 2) != 0,
        },
        format_erases_all_namespaces: fna & (1 << 1) != 0,
    })
}

/// Identify Namespace (CNS 00h), fields used by Oblivio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifyNamespace {
    /// NSZE, bytes 0..8: size in logical blocks.
    pub size_blocks: u64,
    /// Logical block size of the current format, in bytes.
    pub block_bytes: u64,
    /// NGUID, bytes 104..120, lowercase hexadecimal; `None` if zero.
    pub nguid: Option<String>,
    /// EUI64, bytes 120..128, lowercase hexadecimal; `None` if zero.
    pub eui64: Option<String>,
}

impl IdentifyNamespace {
    /// Capacity in bytes, `None` on overflow.
    pub fn capacity_bytes(&self) -> Option<u64> {
        self.size_blocks.checked_mul(self.block_bytes)
    }
}

/// Decodes Identify Namespace.
pub fn identify_namespace(data: &[u8]) -> Result<IdentifyNamespace, DecodeError> {
    require(data, IDENTIFY_SIZE, "NVMe Identify Namespace")?;
    // FLBAS bits 3:0 (and 6:5 for more than 16 formats) select the LBA format.
    let flbas = data[26];
    let format = usize::from(flbas & 0x0f) | (usize::from((flbas >> 5) & 0x03) << 4);
    let lba_format = le_u32(data, 128 + 4 * format);
    let lbads = (lba_format >> 16) & 0xff;
    if !(9..=63).contains(&lbads) {
        return Err(DecodeError::Invalid {
            structure: "NVMe Identify Namespace",
            reason: "logical block size out of range",
        });
    }
    Ok(IdentifyNamespace {
        size_blocks: le_u64(data, 0),
        block_bytes: 1u64 << lbads,
        nguid: hex_id(&data[104..120]),
        eui64: hex_id(&data[120..128]),
    })
}

/// Decodes the SMART / Health Information log page.
pub fn health(data: &[u8]) -> Result<NvmeHealth, DecodeError> {
    require(data, HEALTH_LOG_SIZE, "NVMe SMART / Health log")?;
    Ok(NvmeHealth {
        critical_warning: CriticalWarning(data[0]),
        percentage_used: data[5],
        media_errors: le_u128(data, 160),
    })
}

/// Decodes the Sanitize Status log page: SSTAT bits 2:0 (bytes 2..4).
pub fn sanitize_state(data: &[u8]) -> Result<SanitizeState, DecodeError> {
    require(data, SANITIZE_LOG_SIZE, "NVMe Sanitize Status log")?;
    Ok(match le_u16(data, 2) & 0x07 {
        2 => SanitizeState::InProgress,
        3 => SanitizeState::Failed,
        // 0 never sanitized, 1 and 4 completed.
        _ => SanitizeState::Idle,
    })
}

/// Assembles the engine profile of an NVMe drive.
pub fn profile(
    controller: &IdentifyController,
    health: NvmeHealth,
    sanitize_state: SanitizeState,
) -> NvmeProfile {
    NvmeProfile {
        sanitize: controller.sanitize,
        format: controller.format,
        sanitize_state,
        health,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(buffer: &mut [u8], offset: usize, bytes: &[u8]) {
        buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    fn padded(text: &str, width: usize) -> Vec<u8> {
        format!("{text:<width$}").into_bytes()
    }

    fn controller_buffer() -> Vec<u8> {
        let mut data = vec![0u8; IDENTIFY_SIZE];
        put(&mut data, 4, &padded("241248C8AF57", 20));
        put(&mut data, 24, &padded("Micron MTFDKCD1T0TGE-1BK1AABLA", 40));
        put(&mut data, 64, &padded("1002V6LN", 8));
        put(&mut data, 256, &0b10u16.to_le_bytes()); // OACS: Format NVM
        put(&mut data, 328, &0b011u32.to_le_bytes()); // SANICAP: CES + BES
        data[524] = 0b110; // FNA: secure erase all namespaces, crypto erase
        data
    }

    #[test]
    fn decodes_identify_controller() {
        let controller = identify_controller(&controller_buffer()).expect("valid");
        assert_eq!(controller.serial, "241248C8AF57");
        assert_eq!(controller.model, "Micron MTFDKCD1T0TGE-1BK1AABLA");
        assert_eq!(controller.firmware, "1002V6LN");
        assert_eq!(
            controller.sanitize,
            NvmeSanitizeCaps {
                block_erase: true,
                crypto_erase: true
            }
        );
        assert_eq!(
            controller.format,
            NvmeFormatCaps {
                supported: true,
                crypto_erase: true
            }
        );
        assert!(controller.format_erases_all_namespaces);
    }

    #[test]
    fn sanitize_capabilities_are_independent_bits() {
        let mut data = controller_buffer();
        put(&mut data, 328, &0b100u32.to_le_bytes()); // Overwrite only
        let controller = identify_controller(&data).expect("valid");
        assert_eq!(controller.sanitize, NvmeSanitizeCaps::default());
    }

    #[test]
    fn decodes_identify_namespace() {
        let mut data = vec![0u8; IDENTIFY_SIZE];
        put(&mut data, 0, &2_000_409_264u64.to_le_bytes());
        data[26] = 1; // current format: LBA format 1
        put(&mut data, 128, &(9u32 << 16).to_le_bytes()); // format 0: 512 bytes
        put(&mut data, 132, &(12u32 << 16).to_le_bytes()); // format 1: 4096 bytes
        put(
            &mut data,
            120,
            &[0x00, 0xa0, 0x75, 0x01, 0x48, 0xc8, 0xaf, 0x57],
        );
        let namespace = identify_namespace(&data).expect("valid");
        assert_eq!(namespace.block_bytes, 4096);
        assert_eq!(namespace.capacity_bytes(), Some(2_000_409_264 * 4096));
        assert_eq!(namespace.eui64.as_deref(), Some("00a0750148c8af57"));
        assert_eq!(namespace.nguid, None);
    }

    #[test]
    fn rejects_an_impossible_block_size() {
        let data = vec![0u8; IDENTIFY_SIZE]; // LBADS 0
        assert!(identify_namespace(&data).is_err());
    }

    #[test]
    fn decodes_health_log() {
        let mut data = vec![0u8; HEALTH_LOG_SIZE];
        data[0] = CriticalWarning::RELIABILITY_DEGRADED;
        data[5] = 104;
        put(&mut data, 160, &7u128.to_le_bytes());
        let health = health(&data).expect("valid");
        assert!(
            health
                .critical_warning
                .has(CriticalWarning::RELIABILITY_DEGRADED)
        );
        assert_eq!(health.percentage_used, 104);
        assert_eq!(health.media_errors, 7);
    }

    #[test]
    fn decodes_sanitize_state() {
        let state = |sstat: u16| {
            let mut data = vec![0u8; SANITIZE_LOG_SIZE];
            put(&mut data, 2, &sstat.to_le_bytes());
            sanitize_state(&data).expect("valid")
        };
        assert_eq!(state(0), SanitizeState::Idle);
        assert_eq!(state(1 | 1 << 8), SanitizeState::Idle);
        assert_eq!(state(2), SanitizeState::InProgress);
        assert_eq!(state(3), SanitizeState::Failed);
    }

    #[test]
    fn short_buffers_are_rejected() {
        assert!(identify_controller(&[0; 100]).is_err());
        assert!(health(&[0; 511]).is_err());
        assert!(sanitize_state(&[0; 3]).is_err());
    }
}
