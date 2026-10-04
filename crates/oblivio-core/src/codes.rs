//! Reason codes produced by the decision engine.
//!
//! Codes are the only vocabulary that reaches the signed attestation and the
//! IPC protocol: their string form (`"P1"`, `"R4"`…) is stable and never
//! translated. See `docs/moteur_decision.md`.

/// Declares a code enum with descriptive variants mapped to stable identifiers.
macro_rules! codes {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $code:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $name {
            /// Every code of this family, in ascending order.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )+ ];

            /// Stable identifier, as written in attestations and the IPC protocol.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $code, )+
                }
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

codes! {
    /// Why a drive is PROTECTED and must not be touched (§1).
    ProtectCode {
        /// Hosts `/`, `/boot`, `/boot/efi` or the boot medium.
        SystemDisk = "P1",
        /// A partition is mounted.
        MountedPartition = "P2",
        /// A partition is active swap.
        ActiveSwap = "P3",
        /// Member of an LVM / md RAID / dm-crypt / multipath volume.
        VolumeMember = "P4",
        /// NVMe: another namespace of the same controller is protected.
        SiblingNamespaceProtected = "P5",
    }
}

codes! {
    /// Why a healthy drive is BLOCKED and needs operator action (§2).
    BlockCode {
        /// ATA Security Frozen (IDENTIFY word 128 bit 3).
        SecurityFrozen = "B1",
        /// ATA Security Locked (IDENTIFY word 128 bit 2).
        SecurityLocked = "B2",
        /// ATA Sanitize Freeze Lock active.
        SanitizeFrozen = "B3",
        /// A previous sanitize operation is still in progress.
        SanitizeInProgress = "B4",
        /// A previous sanitize operation failed (Failure mode).
        SanitizeFailed = "B5",
        /// ATA password attempt counter expired (IDENTIFY word 128 bit 4).
        PasswordAttemptsExpired = "B6",
        /// ATA security enabled by someone else: a user password is set but
        /// the drive is not locked (IDENTIFY word 128 bits 1 and 2).
        SecurityEnabled = "B7",
    }
}

codes! {
    /// Why the erasure cannot be proven: SAY NO (§3.1).
    ///
    /// Retired codes N2, N4, N5, N6 and N7 became disposition codes D1 to D5.
    RefusalCode {
        /// Inspection failed: IDENTIFY or admin command I/O error.
        InspectionFailed = "N1",
        /// NVMe Critical Warning bit 3: media is read-only.
        ReadOnlyMedia = "N3",
        /// Post-operation: read errors during validation.
        ValidationReadErrors = "N8",
        /// No executable method, neither Purge nor Clear.
        NoMethodAvailable = "N9",
        /// Post-operation: verification or validation failed after the allowed retries.
        VerificationFailed = "N10",
    }
}

codes! {
    /// Why physical destruction is recommended after a proven erasure (§3.3).
    DispositionCode {
        /// NVMe Critical Warning bit 2: reliability degraded.
        ReliabilityDegraded = "D1",
        /// NVMe Critical Warning bit 4: volatile memory backup failed.
        VolatileBackupFailed = "D2",
        /// ATA SMART RETURN STATUS reports a threshold exceeded.
        SmartFailing = "D3",
        /// ATA attribute 5 (reallocated sectors) at or above the rule threshold.
        ReallocatedAboveThreshold = "D4",
        /// Post-operation: reallocated sector count grew during the operation.
        ReallocationGrowing = "D5",
    }
}

codes! {
    /// Warnings attached to a READY verdict, shown in the UI and on the certificate (§3.2).
    ReserveCode {
        /// NVMe Available Spare below threshold (Critical Warning bit 0).
        SpareBelowThreshold = "R1",
        /// NVMe Percentage Used at or above 100 %.
        EnduranceExhausted = "R2",
        /// NVMe Media and Data Integrity Errors above zero.
        MediaErrors = "R3",
        /// ATA attribute 5 between 1 and the D4 threshold.
        ReallocatedSectors = "R4",
        /// ATA attribute 197 or 198 above zero.
        PendingOrUncorrectable = "R5",
        /// ATA attribute 187 above zero.
        ReportedUncorrectable = "R6",
        /// Host write: reallocated sectors are not covered.
        HostWriteCoverage = "R7",
        /// Model or firmware listed in the Oblivio quirk base.
        KnownQuirk = "R8",
    }
}

impl RefusalCode {
    /// Whether the operator may force an erasure despite this refusal.
    ///
    /// Only N1: the drive could not be inspected but may still accept writes.
    /// N3 accepts no write, N9 has nothing to run, N8 and N10 come too late.
    pub const fn is_forcible(self) -> bool {
        matches!(self, Self::InspectionFailed)
    }

    /// Whether only the evaluation after the operation can produce this code.
    /// `decide()` never emits it.
    pub const fn is_post_operation(self) -> bool {
        matches!(self, Self::ValidationReadErrors | Self::VerificationFailed)
    }
}

impl DispositionCode {
    /// Whether only the evaluation after the operation can produce this code.
    /// `decide()` never emits it.
    pub const fn is_post_operation(self) -> bool {
        matches!(self, Self::ReallocationGrowing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_stable() {
        let all: Vec<&str> = ProtectCode::ALL
            .iter()
            .map(|c| c.as_str())
            .chain(BlockCode::ALL.iter().map(|c| c.as_str()))
            .chain(RefusalCode::ALL.iter().map(|c| c.as_str()))
            .chain(DispositionCode::ALL.iter().map(|c| c.as_str()))
            .chain(ReserveCode::ALL.iter().map(|c| c.as_str()))
            .collect();
        assert_eq!(
            all,
            [
                "P1", "P2", "P3", "P4", "P5", "B1", "B2", "B3", "B4", "B5", "B6", "B7", "N1", "N3",
                "N8", "N9", "N10", "D1", "D2", "D3", "D4", "D5", "R1", "R2", "R3", "R4", "R5",
                "R6", "R7", "R8",
            ]
        );
    }

    #[test]
    fn display_matches_identifier() {
        assert_eq!(ReserveCode::HostWriteCoverage.to_string(), "R7");
    }
}
