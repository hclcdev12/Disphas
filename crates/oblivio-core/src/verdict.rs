//! The single verdict returned by the decision engine (§6).

use crate::codes::{BlockCode, DispositionCode, ProtectCode, RefusalCode, ReserveCode};
use crate::method::{Category, Method, MethodOption};

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
        /// Method the operator may force, capped at Clear, if any.
        /// Built by [`Verdict::say_no`]; see `crate::force`.
        forcible: Option<MethodOption>,
    },
    /// READY (green): a compliant method is available.
    Ready(Ready),
}

impl Verdict {
    /// Builds a SAY NO verdict. It is forcible, with a host write certified
    /// Clear at most, only when every reason is forcible (N1 alone in V1).
    pub fn say_no(reasons: Vec<RefusalCode>) -> Self {
        let forcible = (!reasons.is_empty() && reasons.iter().all(|code| code.is_forcible()))
            .then_some(MethodOption {
                method: Method::HostOverwrite,
                category: Category::Clear,
            });
        Self::SayNo { reasons, forcible }
    }
}

/// Content of a READY verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    /// Preferred method, executed when the operator does not pick another one.
    pub recommended: MethodOption,
    /// Other executable methods, in order of preference.
    pub alternatives: Vec<MethodOption>,
    /// Warnings about the drive itself, whatever the method.
    /// Use [`Ready::reserves_for`] for the full list of a chosen method.
    pub drive_reserves: Vec<ReserveCode>,
    /// What should happen to the drive after a proven erasure.
    pub disposition: Disposition,
}

impl Ready {
    /// Every allowed method, recommended first.
    pub fn options(&self) -> impl Iterator<Item = MethodOption> + '_ {
        core::iter::once(self.recommended).chain(self.alternatives.iter().copied())
    }

    /// The allowed option for `method`, if any.
    pub fn option(&self, method: Method) -> Option<MethodOption> {
        self.options().find(|option| option.method == method)
    }

    /// Every warning to display, acknowledge and certify if `method` is executed,
    /// sorted by code. `None` when the verdict does not allow `method`.
    pub fn reserves_for(&self, method: Method) -> Option<Vec<ReserveCode>> {
        self.option(method)?;
        Some(self.method_reserves(method))
    }

    /// Drive reserves plus those of `method`, sorted by code, without
    /// checking that the verdict allows `method`.
    pub fn method_reserves(&self, method: Method) -> Vec<ReserveCode> {
        let mut reserves = self.drive_reserves.clone();
        if method.is_host_write() {
            reserves.push(ReserveCode::HostWriteCoverage);
            reserves.sort();
        }
        reserves
    }
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
