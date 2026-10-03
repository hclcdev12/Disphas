//! Wire form of the decision codes, methods and categories.
//!
//! These mirror `oblivio-core` on purpose: the protocol does not depend on the
//! engine, so the UI never links decision logic. The helper converts between
//! the two with exhaustive matches.

use serde::{Deserialize, Serialize};

/// Declares a wire code enum serialized as its stable identifier.
macro_rules! wire_codes {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $code:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub enum $name {
            $( $(#[$vmeta])* #[serde(rename = $code)] $variant, )+
        }

        impl $name {
            /// Every code of this family, in ascending order.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )+ ];

            /// Stable identifier, as written on the wire.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $code, )+
                }
            }
        }
    };
}

wire_codes! {
    /// PROTECTED reasons (P).
    ProtectCode {
        /// P1.
        SystemDisk = "P1",
        /// P2.
        MountedPartition = "P2",
        /// P3.
        ActiveSwap = "P3",
        /// P4.
        VolumeMember = "P4",
        /// P5.
        SiblingNamespaceProtected = "P5",
    }
}

wire_codes! {
    /// BLOCKED reasons (B).
    BlockCode {
        /// B1.
        SecurityFrozen = "B1",
        /// B2.
        SecurityLocked = "B2",
        /// B3.
        SanitizeFrozen = "B3",
        /// B4.
        SanitizeInProgress = "B4",
        /// B5.
        SanitizeFailed = "B5",
        /// B6.
        PasswordAttemptsExpired = "B6",
    }
}

wire_codes! {
    /// SAY NO reasons (N).
    RefusalCode {
        /// N1.
        InspectionFailed = "N1",
        /// N3.
        ReadOnlyMedia = "N3",
        /// N8.
        ValidationReadErrors = "N8",
        /// N9.
        NoMethodAvailable = "N9",
        /// N10.
        VerificationFailed = "N10",
    }
}

wire_codes! {
    /// Disposition reasons (D).
    DispositionCode {
        /// D1.
        ReliabilityDegraded = "D1",
        /// D2.
        VolatileBackupFailed = "D2",
        /// D3.
        SmartFailing = "D3",
        /// D4.
        ReallocatedAboveThreshold = "D4",
        /// D5.
        ReallocationGrowing = "D5",
    }
}

wire_codes! {
    /// Reserves (R).
    ReserveCode {
        /// R1.
        SpareBelowThreshold = "R1",
        /// R2.
        EnduranceExhausted = "R2",
        /// R3.
        MediaErrors = "R3",
        /// R4.
        ReallocatedSectors = "R4",
        /// R5.
        PendingOrUncorrectable = "R5",
        /// R6.
        ReportedUncorrectable = "R6",
        /// R7.
        HostWriteCoverage = "R7",
        /// R8.
        KnownQuirk = "R8",
    }
}

wire_codes! {
    /// Warnings about the scope of an operation, beyond the drive codes.
    ScopeWarning {
        /// An NVMe Sanitize erases every namespace of the controller.
        AllNamespaces = "NS_ALL",
    }
}

/// NIST SP 800-88 category. Never translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Category {
    /// Clear.
    Clear,
    /// Purge.
    Purge,
}

/// Sanitization method, named as in `oblivio-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Method {
    /// NVMe Sanitize, Block Erase.
    NvmeSanitizeBlockErase,
    /// NVMe Sanitize, Crypto Erase.
    NvmeSanitizeCryptoErase,
    /// NVMe Format NVM, SES = 2.
    NvmeFormatCryptoErase,
    /// NVMe Format NVM, SES = 1.
    NvmeFormatUserDataErase,
    /// ATA Sanitize, Block Erase.
    AtaSanitizeBlockErase,
    /// ATA Sanitize, Crypto Scramble.
    AtaSanitizeCryptoScramble,
    /// ATA Sanitize, Overwrite.
    AtaSanitizeOverwrite,
    /// ATA Security Erase Unit, enhanced.
    AtaSecurityEraseEnhanced,
    /// ATA Security Erase Unit, normal.
    AtaSecurityEraseNormal,
    /// Host overwrite, one pass of zeros.
    HostOverwrite,
}

/// A warning the operator must acknowledge before `Sanitize`, e.g. `"D4"`, `"R4"`, `"NS_ALL"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Acknowledgement {
    /// A disposition code.
    Disposition(DispositionCode),
    /// A reserve code.
    Reserve(ReserveCode),
    /// A scope warning.
    Scope(ScopeWarning),
}
