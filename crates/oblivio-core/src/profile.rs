//! Hardware profile: the decision engine's only input.
//!
//! Built by `oblivio-device` from IDENTIFY data, health logs and system state.
//! Fields hold decoded facts, never raw buffers: the engine performs no parsing
//! and no I/O.

/// Everything the engine needs to know about one drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Hardware identity of the drive.
    pub identity: DeviceIdentity,
    /// System-level facts that make the drive untouchable (§1).
    pub protection: ProtectionFacts,
    /// Result of the firmware inspection.
    pub inspection: Inspection,
    /// Number of failed attempts to resolve a blocking state (§2, N9).
    pub failed_unblock_attempts: u32,
    /// Matches in the Oblivio quirk base.
    pub quirks: QuirkFlags,
}

/// Identity of a drive. The helper never identifies a drive by its `/dev` path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceIdentity {
    /// Model string.
    pub model: String,
    /// Full serial number.
    pub serial: String,
    /// Firmware revision.
    pub firmware: String,
    /// ATA World Wide Name, if exposed.
    pub wwn: Option<String>,
    /// NVMe IEEE EUI-64, if exposed.
    pub eui64: Option<String>,
    /// NVMe Namespace Globally Unique Identifier, if exposed.
    pub nguid: Option<String>,
}

/// System state checked by the helper before anything else (§1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProtectionFacts {
    /// P1: hosts `/`, `/boot`, `/boot/efi` or the boot medium.
    pub hosts_system: bool,
    /// P2: a partition is mounted.
    pub has_mounted_partition: bool,
    /// P3: a partition is active swap.
    pub has_active_swap: bool,
    /// P4: member of an LVM / md RAID / dm-crypt / multipath volume.
    pub is_volume_member: bool,
    /// P5: NVMe only, another namespace of the same controller matches P1 to P4.
    pub sibling_namespace_protected: bool,
}

/// Outcome of the firmware inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inspection {
    /// IDENTIFY failed or admin commands returned I/O errors (N1).
    Failed,
    /// NVMe drive.
    Nvme(NvmeProfile),
    /// SATA drive.
    Ata(AtaProfile),
}

/// State of the sanitize engine, read from the sanitize status log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SanitizeState {
    /// No operation in progress, last one (if any) succeeded.
    #[default]
    Idle,
    /// A sanitize operation is in progress (B4).
    InProgress,
    /// The last sanitize operation failed (B5).
    Failed,
}

/// NVMe drive facts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NvmeProfile {
    /// Sanitize capabilities (Identify Controller `SANICAP`).
    pub sanitize: NvmeSanitizeCaps,
    /// Format NVM capabilities.
    pub format: NvmeFormatCaps,
    /// Sanitize status log.
    pub sanitize_state: SanitizeState,
    /// SMART / Health Information log.
    pub health: NvmeHealth,
}

/// NVMe `SANICAP` bits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NvmeSanitizeCaps {
    /// `SANICAP.BES`: Block Erase supported.
    pub block_erase: bool,
    /// `SANICAP.CES`: Crypto Erase supported.
    pub crypto_erase: bool,
}

/// NVMe Format NVM support.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NvmeFormatCaps {
    /// Format NVM command supported (`OACS` bit 1).
    pub supported: bool,
    /// Cryptographic erase supported as a secure erase setting (`FNA` bit 2).
    pub crypto_erase: bool,
}

/// NVMe SMART / Health Information log fields used by the engine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NvmeHealth {
    /// Critical Warning byte.
    pub critical_warning: CriticalWarning,
    /// Percentage Used; may exceed 100.
    pub percentage_used: u8,
    /// Media and Data Integrity Errors.
    pub media_errors: u128,
}

/// NVMe Critical Warning byte.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CriticalWarning(pub u8);

impl CriticalWarning {
    /// Bit 0: available spare below threshold (R1).
    pub const SPARE_BELOW_THRESHOLD: u8 = 1 << 0;
    /// Bit 2: NVM subsystem reliability degraded (D1).
    pub const RELIABILITY_DEGRADED: u8 = 1 << 2;
    /// Bit 3: media placed in read-only mode (N3).
    pub const READ_ONLY: u8 = 1 << 3;
    /// Bit 4: volatile memory backup device failed (D2).
    pub const VOLATILE_BACKUP_FAILED: u8 = 1 << 4;

    /// Whether every bit of `mask` is set.
    pub const fn has(self, mask: u8) -> bool {
        self.0 & mask == mask
    }
}

/// SATA drive facts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AtaProfile {
    /// Non-rotational medium (IDENTIFY word 217 = 1).
    pub solid_state: bool,
    /// Sanitize feature set capabilities (IDENTIFY word 59).
    pub sanitize: AtaSanitizeCaps,
    /// Sanitize Freeze Lock active (B3).
    pub sanitize_frozen: bool,
    /// Sanitize status.
    pub sanitize_state: SanitizeState,
    /// Security feature set (IDENTIFY word 128).
    pub security: AtaSecurity,
    /// SMART status and attributes.
    pub smart: AtaSmart,
}

/// ATA Sanitize capabilities (IDENTIFY word 59).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AtaSanitizeCaps {
    /// Bit 15: Block Erase supported.
    pub block_erase: bool,
    /// Bit 14: Overwrite supported.
    pub overwrite: bool,
    /// Bit 13: Crypto Scramble supported.
    pub crypto_scramble: bool,
}

/// ATA Security feature set (IDENTIFY word 128).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AtaSecurity {
    /// Bit 0: Security feature set supported.
    pub supported: bool,
    /// Bit 5: Enhanced Security Erase supported.
    pub enhanced_erase: bool,
    /// Bit 2: Locked (B2).
    pub locked: bool,
    /// Bit 3: Frozen (B1).
    pub frozen: bool,
    /// Bit 4: password attempt counter expired (B6).
    pub attempts_expired: bool,
}

/// ATA SMART data used by the engine. Attributes are `None` when not reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AtaSmart {
    /// SMART RETURN STATUS reports a threshold exceeded (D3).
    pub failing: bool,
    /// Attribute 5: reallocated sectors (R4, D4).
    pub reallocated_sectors: Option<u64>,
    /// Attribute 187: reported uncorrectable errors (R6).
    pub reported_uncorrectable: Option<u64>,
    /// Attribute 197: current pending sectors (R5).
    pub pending_sectors: Option<u64>,
    /// Attribute 198: offline uncorrectable sectors (R5).
    pub offline_uncorrectable: Option<u64>,
}

/// Matches in the Oblivio quirk base for this model and firmware.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuirkFlags {
    /// Listed with a known issue (R8).
    pub known_issue: bool,
    /// ATA Security Erase validated as Purge on this SSD model and firmware.
    pub security_erase_purge_validated: bool,
}
