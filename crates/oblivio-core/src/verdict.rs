//! The single verdict returned by the decision engine (§6).

use crate::codes::{BlockCode, DispositionCode, ProtectCode, RefusalCode, ReserveCode};
use crate::method::MethodOption;

/// Decision for one drive. Exactly one state, never a combination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// PROTECTED (grey): the drive must not be touched.
    Protected {
        /// Every protection that applies.
        reasons: Vec<ProtectCode>,
    },
    /// BLOCKED (orange): healthy drive, operator action needed first.
    Blocked {
        /// Every blocking condition that prevents the preferred method.
        reasons: Vec<BlockCode>,
    },
    /// SAY NO (red): the erasure cannot be proven.
    SayNo {
        /// Every refusal reason found.
        reasons: Vec<RefusalCode>,
    },
    /// READY (green): a compliant method is available.
    Ready(Ready),
}

/// Content of a READY verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    /// Preferred method, executed when the operator does not pick another one.
    pub recommended: MethodOption,
    /// Other executable methods, in order of preference.
    pub alternatives: Vec<MethodOption>,
    /// Warnings for the recommended method, displayed and certified.
    pub reserves: Vec<ReserveCode>,
    /// What should happen to the drive after a proven erasure.
    pub disposition: Disposition,
}

/// Recommended future of the drive once its erasure is certified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// The drive can be reused.
    Reuse,
    /// The erasure is certified, but physical destruction is recommended.
    DestroyRecommended {
        /// Every disposition reason found.
        reasons: Vec<DispositionCode>,
    },
}

/// Operator guidance for a blocking condition (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guidance {
    /// Suspend to RAM (S3), resume, then inspect again.
    SuspendAndResume,
    /// Enter the ATA user or master password.
    EnterPassword,
    /// Power-cycle the drive.
    PowerCycle,
    /// Wait for the running operation; it resumes automatically.
    WaitForCompletion,
    /// Send Exit Failure Mode, then retry.
    ExitFailureMode,
}

impl BlockCode {
    /// Guidance offered to the operator for this condition.
    pub const fn guidance(self) -> Guidance {
        match self {
            Self::SecurityFrozen => Guidance::SuspendAndResume,
            Self::SecurityLocked => Guidance::EnterPassword,
            Self::SanitizeFrozen | Self::PasswordAttemptsExpired => Guidance::PowerCycle,
            Self::SanitizeInProgress => Guidance::WaitForCompletion,
            Self::SanitizeFailed => Guidance::ExitFailureMode,
        }
    }
}
