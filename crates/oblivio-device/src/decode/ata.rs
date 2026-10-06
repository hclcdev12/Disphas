//! ATA data structures (ACS-3 / ACS-4).
//!
//! IDENTIFY DEVICE is 256 little-endian words; strings swap the two bytes of
//! each word. To be cross-checked against `smartctl --json -x` on the test
//! bench (`docs/cadre_projet.md` §9).

use oblivio_core::profile::{AtaProfile, AtaSanitizeCaps, AtaSecurity, AtaSmart, SanitizeState};

use super::{DecodeError, require};

/// Size of IDENTIFY DEVICE data and of SMART READ DATA.
pub const SECTOR_SIZE: usize = 512;

/// IDENTIFY DEVICE, fields used by Oblivio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifyDevice {
    /// Words 10..20.
    pub serial: String,
    /// Words 23..27.
    pub firmware: String,
    /// Words 27..47.
    pub model: String,
    /// Words 108..112, lowercase hexadecimal, if word 87 bit 8 is set.
    pub wwn: Option<String>,
    /// Words 100..104 times the logical sector size (words 106, 117..119).
    pub capacity_bytes: u64,
    /// Word 217: `Some(true)` non-rotating, `Some(false)` rotating, `None` not reported.
    pub solid_state: Option<bool>,
    /// Word 59 bit 12: Sanitize feature set supported.
    pub sanitize_supported: bool,
    /// Word 59 bits 15, 14, 13.
    pub sanitize: AtaSanitizeCaps,
    /// Word 128.
    pub security: AtaSecurity,
    /// Word 89: estimated SECURITY ERASE UNIT time, normal mode, in minutes.
    pub security_erase_minutes: Option<u32>,
    /// Word 90: estimated SECURITY ERASE UNIT time, enhanced mode, in minutes.
    pub enhanced_erase_minutes: Option<u32>,
}

/// Estimate used for "more than 508 minutes" (255 in the short format):
/// the real time has no upper bound, so assume 36 hours, which makes the
/// command timeout (twice the estimate) 72 hours rather than 17.
const BEYOND_SHORT_FORMAT_MINUTES: u32 = 36 * 60;

/// Erase time of IDENTIFY words 89/90 (ACS-3 §7.12.7.30): bit 15 set, bits
/// 14:0 × 2 minutes; otherwise bits 7:0 × 2 minutes, 0 meaning unknown and
/// 255 meaning more than 508 minutes (not 510). In the extended format,
/// 32767 ("more than 65532 minutes") already saturates the timeout.
fn erase_minutes(word: u16) -> Option<u32> {
    if word & 0x8000 != 0 {
        let value = u32::from(word & 0x7fff);
        return (value != 0).then_some(value * 2);
    }
    match word & 0x00ff {
        0 => None,
        0xff => Some(BEYOND_SHORT_FORMAT_MINUTES),
        value => Some(u32::from(value) * 2),
    }
}

/// Decodes IDENTIFY DEVICE, checking its integrity word when present.
pub fn identify_device(data: &[u8]) -> Result<IdentifyDevice, DecodeError> {
    require(data, SECTOR_SIZE, "ATA IDENTIFY DEVICE")?;
    let word = |index: usize| u16::from_le_bytes([data[2 * index], data[2 * index + 1]]);
    let bit = |index: usize, bit: u32| word(index) & (1 << bit) != 0;

    // Word 255: signature A5h in bits 7:0, checksum in bits 15:8.
    if data[510] == 0xa5
        && data
            .iter()
            .take(SECTOR_SIZE)
            .fold(0u8, |sum, b| sum.wrapping_add(*b))
            != 0
    {
        return Err(DecodeError::Invalid {
            structure: "ATA IDENTIFY DEVICE",
            reason: "integrity checksum mismatch",
        });
    }

    let lbas = (0..4).fold(0u64, |acc, i| acc | (u64::from(word(100 + i)) << (16 * i)));
    // Word 106: bit 14 set and bit 15 clear means valid; bit 12 means words 117..118 hold the size in words.
    let sector_bytes = if word(106) & 0xc000 == 0x4000 && bit(106, 12) {
        2 * (u64::from(word(117)) | (u64::from(word(118)) << 16))
    } else {
        512
    };
    let wwn = bit(87, 8).then(|| {
        (108..112).fold(String::new(), |mut text, index| {
            text.push_str(&format!("{:04x}", word(index)));
            text
        })
    });
    let wwn = wwn.filter(|text| text.bytes().any(|b| b != b'0'));

    Ok(IdentifyDevice {
        serial: swapped_ascii(&data[20..40]),
        firmware: swapped_ascii(&data[46..54]),
        model: swapped_ascii(&data[54..94]),
        wwn,
        capacity_bytes: lbas.saturating_mul(sector_bytes),
        solid_state: match word(217) {
            0 => None,
            1 => Some(true),
            _ => Some(false),
        },
        sanitize_supported: bit(59, 12),
        sanitize: AtaSanitizeCaps {
            block_erase: bit(59, 12) && bit(59, 15),
            overwrite: bit(59, 12) && bit(59, 14),
            crypto_scramble: bit(59, 12) && bit(59, 13),
        },
        security_erase_minutes: erase_minutes(word(89)),
        enhanced_erase_minutes: erase_minutes(word(90)),
        security: AtaSecurity {
            supported: bit(128, 0),
            locked: bit(128, 2),
            frozen: bit(128, 3),
            attempts_expired: bit(128, 4),
            enhanced_erase: bit(128, 5),
            enabled: bit(128, 1),
        },
    })
}

/// Raw values of the SMART attributes used by the engine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmartAttributes {
    /// Attribute 5.
    pub reallocated_sectors: Option<u64>,
    /// Attribute 187.
    pub reported_uncorrectable: Option<u64>,
    /// Attribute 197.
    pub pending_sectors: Option<u64>,
    /// Attribute 198.
    pub offline_uncorrectable: Option<u64>,
}

/// Decodes SMART READ DATA: 30 attributes of 12 bytes from byte 2, checksum in byte 511.
///
/// Raw values are read as 48-bit integers, as `smartctl` does by default.
/// Vendors that pack other data in the upper bytes inflate the value, which
/// errs on the safe side (D4); the quirk base will correct known models.
pub fn smart_attributes(data: &[u8]) -> Result<SmartAttributes, DecodeError> {
    require(data, SECTOR_SIZE, "ATA SMART READ DATA")?;
    if data
        .iter()
        .take(SECTOR_SIZE)
        .fold(0u8, |sum, b| sum.wrapping_add(*b))
        != 0
    {
        return Err(DecodeError::Invalid {
            structure: "ATA SMART READ DATA",
            reason: "checksum mismatch",
        });
    }
    let mut attributes = SmartAttributes::default();
    for entry in data[2..2 + 30 * 12].chunks_exact(12) {
        let raw = entry[5..11]
            .iter()
            .rev()
            .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte));
        let slot = match entry[0] {
            5 => &mut attributes.reallocated_sectors,
            187 => &mut attributes.reported_uncorrectable,
            197 => &mut attributes.pending_sectors,
            198 => &mut attributes.offline_uncorrectable,
            _ => continue,
        };
        *slot = Some(raw);
    }
    Ok(attributes)
}

/// Decodes SMART RETURN STATUS from the LBA Mid and LBA High output registers:
/// `false` for 4Fh/C2h (no threshold exceeded), `true` for F4h/2Ch.
pub fn smart_failing(lba_mid: u8, lba_high: u8) -> Result<bool, DecodeError> {
    match (lba_mid, lba_high) {
        (0x4f, 0xc2) => Ok(false),
        (0xf4, 0x2c) => Ok(true),
        _ => Err(DecodeError::Invalid {
            structure: "ATA SMART RETURN STATUS",
            reason: "unexpected register values",
        }),
    }
}

/// State reported by SANITIZE STATUS EXT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SanitizeStatus {
    /// Operation state.
    pub state: SanitizeState,
    /// Sanitize Freeze Lock active (B3).
    pub frozen: bool,
}

/// Decodes SANITIZE STATUS EXT: Count output bit 14 in progress, bit 13
/// frozen; an abort reporting a failed sanitize means Failure mode.
pub fn sanitize_status(count: u16, failed: bool) -> SanitizeStatus {
    let state = if failed {
        SanitizeState::Failed
    } else if count & (1 << 14) != 0 {
        SanitizeState::InProgress
    } else {
        SanitizeState::Idle
    };
    SanitizeStatus {
        state,
        frozen: count & (1 << 13) != 0,
    }
}

/// Assembles the engine profile of a SATA drive.
///
/// A drive whose word 217 does not report its media is treated as an SSD,
/// which never offers Sanitize Overwrite and certifies Security Erase as
/// Clear only. The kernel's `queue/rotational` is no help: for a SATA drive
/// libata derives it from the same word 217 and reports "rotational" when it
/// is absent (observed on the test bench, 2026-10-06).
pub fn profile(
    identify: &IdentifyDevice,
    smart: Option<(bool, SmartAttributes)>,
    sanitize: SanitizeStatus,
) -> AtaProfile {
    let solid_state = identify.solid_state.unwrap_or(true);
    let media_type_unknown = identify.solid_state.is_none();
    let (failing, attributes) = smart.unwrap_or_default();
    AtaProfile {
        solid_state,
        media_type_unknown,
        sanitize: identify.sanitize,
        sanitize_frozen: sanitize.frozen,
        sanitize_state: sanitize.state,
        security: identify.security,
        smart: AtaSmart {
            failing,
            reallocated_sectors: attributes.reallocated_sectors,
            reported_uncorrectable: attributes.reported_uncorrectable,
            pending_sectors: attributes.pending_sectors,
            offline_uncorrectable: attributes.offline_uncorrectable,
        },
    }
}

/// ATA string: bytes swapped within each word, then trimmed.
fn swapped_ascii(bytes: &[u8]) -> String {
    let swapped: Vec<u8> = bytes
        .chunks_exact(2)
        .flat_map(|pair| [pair[1], pair[0]])
        .collect();
    super::ascii(&swapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a 512-byte IDENTIFY DEVICE buffer word by word.
    struct Identify([u16; 256]);

    impl Identify {
        fn new() -> Self {
            Self([0; 256])
        }

        fn set(mut self, index: usize, value: u16) -> Self {
            self.0[index] = value;
            self
        }

        fn text(mut self, first: usize, words: usize, text: &str) -> Self {
            let bytes = format!("{text:<width$}", width = 2 * words).into_bytes();
            for (offset, pair) in bytes.chunks_exact(2).enumerate() {
                self.0[first + offset] = u16::from_be_bytes([pair[0], pair[1]]);
            }
            self
        }

        fn bytes(&self) -> Vec<u8> {
            self.0.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        /// Sets the integrity word so the checksum is valid.
        fn signed(self) -> Vec<u8> {
            let mut data = self.set(255, 0x00a5).bytes();
            let sum = data.iter().fold(0u8, |sum, b| sum.wrapping_add(*b));
            data[511] = 0u8.wrapping_sub(sum);
            data
        }
    }

    fn ssd() -> Identify {
        Identify::new()
            .text(10, 10, "S3Z9NB0K123456A")
            .text(23, 4, "RVT04B6Q")
            .text(27, 20, "Samsung SSD 860 EVO 500GB")
            .set(59, 0b1011 << 12) // Sanitize, Crypto Scramble, Block Erase
            .set(87, 1 << 8)
            .set(100, 0x6030)
            .set(101, 0x3a38) // 976 773 168 sectors
            .set(108, 0x5002)
            .set(109, 0x538e)
            .set(110, 0x4012)
            .set(111, 0x3456)
            .set(128, 0b10_1001) // supported, frozen, enhanced erase
            .set(217, 1)
    }

    #[test]
    fn decodes_identify_device() {
        let identify = identify_device(&ssd().signed()).expect("valid");
        assert_eq!(identify.serial, "S3Z9NB0K123456A");
        assert_eq!(identify.firmware, "RVT04B6Q");
        assert_eq!(identify.model, "Samsung SSD 860 EVO 500GB");
        assert_eq!(identify.wwn.as_deref(), Some("5002538e40123456"));
        assert_eq!(identify.capacity_bytes, 976_773_168 * 512);
        assert_eq!(identify.solid_state, Some(true));
        assert_eq!(
            identify.sanitize,
            AtaSanitizeCaps {
                block_erase: true,
                overwrite: false,
                crypto_scramble: true
            }
        );
        assert_eq!(
            identify.security,
            AtaSecurity {
                supported: true,
                enhanced_erase: true,
                enabled: false,
                locked: false,
                frozen: true,
                attempts_expired: false
            }
        );
    }

    #[test]
    fn sanitize_bits_need_the_feature_set_bit() {
        let identify = identify_device(&ssd().set(59, 0b1111 << 12).signed()).expect("valid");
        assert!(identify.sanitize_supported);
        // Erase bits set without bit 12 (Sanitize feature set) are ignored.
        let identify = identify_device(&ssd().set(59, 0b1110 << 12).signed()).expect("valid");
        assert!(!identify.sanitize_supported);
        assert_eq!(identify.sanitize, AtaSanitizeCaps::default());
    }

    #[test]
    fn reads_erase_time_estimates() {
        let identify =
            identify_device(&ssd().set(89, 0x0019).set(90, 0x8000 | 300).signed()).expect("valid");
        assert_eq!(identify.security_erase_minutes, Some(50));
        assert_eq!(identify.enhanced_erase_minutes, Some(600));
        let unknown = identify_device(&ssd().signed()).expect("valid");
        assert_eq!(unknown.security_erase_minutes, None);
        // 255: "more than 508 minutes", never read as 510.
        let long = identify_device(&ssd().set(89, 0x00ff).signed()).expect("valid");
        assert_eq!(long.security_erase_minutes, Some(36 * 60));
        assert_eq!(
            crate::security::erase_timeout_ms(long.security_erase_minutes),
            72 * 3_600_000
        );
    }

    #[test]
    fn rotation_rate_and_logical_sector_size() {
        let hdd = ssd()
            .set(217, 7200)
            .set(106, 0x4000 | 1 << 12)
            .set(117, 2048); // 4096-byte logical sectors
        let identify = identify_device(&hdd.signed()).expect("valid");
        assert_eq!(identify.solid_state, Some(false));
        assert_eq!(identify.capacity_bytes, 976_773_168 * 4096);
        let unknown = identify_device(&ssd().set(217, 0).signed()).expect("valid");
        assert_eq!(unknown.solid_state, None);
    }

    #[test]
    fn corrupted_identify_is_rejected() {
        let mut data = ssd().signed();
        data[200] ^= 0xff;
        assert!(identify_device(&data).is_err());
        // Without the A5h signature, there is no checksum to verify.
        assert!(identify_device(&ssd().bytes()).is_ok());
    }

    fn smart_buffer(attributes: &[(u8, u64)]) -> Vec<u8> {
        let mut data = vec![0u8; SECTOR_SIZE];
        data[0] = 0x10; // revision
        for (slot, (id, raw)) in attributes.iter().enumerate() {
            let entry = &mut data[2 + 12 * slot..2 + 12 * (slot + 1)];
            entry[0] = *id;
            entry[3] = 100;
            entry[4] = 100;
            entry[5..11].copy_from_slice(&raw.to_le_bytes()[..6]);
        }
        let sum = data.iter().fold(0u8, |sum, b| sum.wrapping_add(*b));
        data[511] = 0u8.wrapping_sub(sum);
        data
    }

    #[test]
    fn decodes_smart_attributes() {
        let data = smart_buffer(&[(9, 12_000), (5, 12), (197, 1), (198, 0), (187, 3)]);
        assert_eq!(
            smart_attributes(&data).expect("valid"),
            SmartAttributes {
                reallocated_sectors: Some(12),
                reported_uncorrectable: Some(3),
                pending_sectors: Some(1),
                offline_uncorrectable: Some(0),
            }
        );
    }

    #[test]
    fn raw_values_are_48_bit() {
        let data = smart_buffer(&[(5, 0x0001_0000_0002)]);
        assert_eq!(
            smart_attributes(&data).expect("valid").reallocated_sectors,
            Some(0x0001_0000_0002)
        );
    }

    #[test]
    fn missing_attributes_stay_unknown() {
        let data = smart_buffer(&[(9, 1)]);
        assert_eq!(
            smart_attributes(&data).expect("valid"),
            SmartAttributes::default()
        );
    }

    #[test]
    fn corrupted_smart_data_is_rejected() {
        let mut data = smart_buffer(&[(5, 12)]);
        data[7] ^= 0x01;
        assert!(smart_attributes(&data).is_err());
    }

    #[test]
    fn decodes_smart_return_status() {
        assert_eq!(smart_failing(0x4f, 0xc2), Ok(false));
        assert_eq!(smart_failing(0xf4, 0x2c), Ok(true));
        assert!(smart_failing(0, 0).is_err());
    }

    #[test]
    fn decodes_sanitize_status() {
        assert_eq!(sanitize_status(1 << 15, false).state, SanitizeState::Idle);
        assert_eq!(
            sanitize_status(1 << 14, false).state,
            SanitizeState::InProgress
        );
        assert_eq!(sanitize_status(0, true).state, SanitizeState::Failed);
        assert!(sanitize_status(1 << 13, false).frozen);
    }

    #[test]
    fn unknown_media_type_is_treated_as_ssd() {
        // Word 217 = 0, as on the bench's Toshiba MK1655GSX.
        let identify = identify_device(&ssd().set(217, 0).signed()).expect("valid");
        let idle = sanitize_status(0, false);
        assert!(profile(&identify, None, idle).solid_state);
        assert!(
            profile(&identify, None, idle).media_type_unknown,
            "never certified as an SSD"
        );
        let rotating = identify_device(&ssd().set(217, 7200).signed()).expect("valid");
        assert!(!profile(&rotating, None, idle).solid_state);
        let solid = identify_device(&ssd().set(217, 1).signed()).expect("valid");
        assert!(profile(&solid, None, idle).solid_state);
        assert!(!profile(&solid, None, idle).media_type_unknown);
        assert!(!profile(&rotating, None, idle).media_type_unknown);
    }
}
