//! Sanitization methods and NIST SP 800-88 categories.

/// NIST SP 800-88 sanitization category.
///
/// *Destruct* is deliberately absent: Oblivio never executes it, it only
/// prescribes or recommends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    /// Logical techniques against simple, non-invasive recovery.
    Clear,
    /// Firmware techniques against laboratory recovery.
    Purge,
}

/// A sanitization method Oblivio can execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    /// NVMe Sanitize, Block Erase action.
    NvmeSanitizeBlockErase,
    /// NVMe Sanitize, Crypto Erase action.
    NvmeSanitizeCryptoErase,
    /// NVMe Format NVM, Secure Erase Setting 2 (cryptographic erase).
    NvmeFormatCryptoErase,
    /// NVMe Format NVM, Secure Erase Setting 1 (user data erase).
    NvmeFormatUserDataErase,
    /// ATA Sanitize, Block Erase.
    AtaSanitizeBlockErase,
    /// ATA Sanitize, Crypto Scramble.
    AtaSanitizeCryptoScramble,
    /// ATA Sanitize, Overwrite.
    AtaSanitizeOverwrite,
    /// ATA Security Erase Unit, enhanced mode.
    AtaSecurityEraseEnhanced,
    /// ATA Security Erase Unit, normal mode.
    AtaSecurityEraseNormal,
    /// Single-pass zero overwrite from the host. Never a Purge.
    HostOverwrite,
}

impl Method {
    /// Whether the data is written by the host rather than erased by firmware.
    pub const fn is_host_write(self) -> bool {
        matches!(self, Self::HostOverwrite)
    }

    /// Whether the method uses the ATA Sanitize feature set.
    pub const fn is_ata_sanitize(self) -> bool {
        matches!(
            self,
            Self::AtaSanitizeBlockErase
                | Self::AtaSanitizeCryptoScramble
                | Self::AtaSanitizeOverwrite
        )
    }

    /// Whether the method uses the ATA Security feature set.
    pub const fn is_ata_security_erase(self) -> bool {
        matches!(
            self,
            Self::AtaSecurityEraseEnhanced | Self::AtaSecurityEraseNormal
        )
    }
}

/// A method together with the category it would be certified under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodOption {
    /// The method.
    pub method: Method,
    /// Its certified category.
    pub category: Category,
}
