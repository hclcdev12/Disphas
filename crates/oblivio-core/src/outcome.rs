//! Post-operation evaluation: turns the result of an executed method into the
//! final outcome, or asks for one more attempt.
//!
//! Produces the post-operation codes N8, N10 and D5 of
//! `docs/moteur_decision.md` §3. Like `decide()`, it is pure and deterministic.
//! A forced attempt is held to the same checks but can never be compliant.

use thiserror::Error;

use crate::codes::{DispositionCode, RefusalCode, ReserveCode};
use crate::force::Forced;
use crate::method::{Method, MethodOption};
use crate::profile::{AtaSmart, DeviceProfile, Inspection};
use crate::rules::RuleSet;
use crate::verdict::{Disposition, Ready};

/// What the helper observed after one execution of the method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttemptReport {
    /// The firmware reported success (command completion, sanitize status log).
    /// Always `true` for a host write that completed.
    pub verification_passed: bool,
    /// Result of the sample validation (§5).
    pub validation: Validation,
    /// SMART data read again after the operation; `None` for NVMe.
    pub smart_after: Option<AtaSmart>,
}

/// Result of the sample validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Validation {
    /// Every sample matches: expected pattern, or no canary left.
    Passed,
    /// At least one sample does not match.
    Mismatch,
    /// At least one sample could not be read (N8).
    ReadErrors,
}

/// What to do after an attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluation {
    /// Run the method again; retries are still allowed.
    Retry,
    /// Erasure proven: certificate status CONFORME.
    Compliant(Compliant),
    /// Forced erasure completed and checked: certificate status NON CONFORME,
    /// physical destruction required.
    NonCompliant(NonCompliant),
    /// Erasure not provable: refusal report.
    Refused {
        /// Every refusal reason found, sorted by code.
        reasons: Vec<RefusalCode>,
    },
}

/// Content of a proven erasure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compliant {
    /// Method executed and the category it is certified under.
    pub executed: MethodOption,
    /// Every warning certified with the erasure.
    pub reserves: Vec<ReserveCode>,
    /// Recommended future of the drive, including post-operation codes.
    pub disposition: Disposition,
}

/// Content of a forced erasure that passed its checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonCompliant {
    /// Method executed, certified Clear at most.
    pub executed: MethodOption,
    /// Refusal reasons of the forced verdict.
    pub refusals: Vec<RefusalCode>,
    /// Every warning certified with the erasure.
    pub reserves: Vec<ReserveCode>,
    /// Operator justification, kept exactly as typed.
    pub justification: String,
}

/// Why an attempt cannot be evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvaluationError {
    /// The executed method is not allowed by the verdict.
    #[error("method {0:?} is not allowed by the verdict")]
    MethodNotAllowed(Method),
}

/// Evaluates one attempt of `executed` on the drive described by `profile`,
/// whose READY verdict was `ready`. `retries_used` counts the attempts already
/// repeated before this one.
pub fn evaluate_attempt(
    profile: &DeviceProfile,
    ready: &Ready,
    executed: Method,
    retries_used: u8,
    report: &AttemptReport,
    rules: &RuleSet,
) -> Result<Evaluation, EvaluationError> {
    let executed = ready
        .option(executed)
        .ok_or(EvaluationError::MethodNotAllowed(executed))?;
    let smart_before = match &profile.inspection {
        Inspection::Ata(ata) => Some(ata.smart),
        Inspection::Nvme(_) | Inspection::Failed => None,
    };

    if let Some(failure) = failure(report, retries_used, rules) {
        return Ok(failure.into_evaluation(Vec::new()));
    }

    // D5: the drive kept reallocating sectors while being erased.
    let reallocated = |smart: Option<AtaSmart>| smart.and_then(|smart| smart.reallocated_sectors);
    let growing = matches!(
        (reallocated(smart_before), reallocated(report.smart_after)),
        (Some(before), Some(after)) if after > before
    );
    let disposition = match (&ready.disposition, growing) {
        (disposition, false) => disposition.clone(),
        (Disposition::Reuse, true) => Disposition::DestroyRecommended {
            reasons: vec![DispositionCode::ReallocationGrowing],
        },
        (Disposition::DestroyRecommended { reasons }, true) => {
            let mut reasons = reasons.clone();
            reasons.push(DispositionCode::ReallocationGrowing);
            reasons.sort();
            Disposition::DestroyRecommended { reasons }
        }
    };

    Ok(Evaluation::Compliant(Compliant {
        executed,
        reserves: ready
            .reserves_for(executed.method)
            .ok_or(EvaluationError::MethodNotAllowed(executed.method))?,
        disposition,
    }))
}

/// Evaluates one attempt of a forced erasure. Success is NON CONFORME at best;
/// a failed check still yields a refusal (REFUS) carrying the original reasons.
pub fn evaluate_forced_attempt(
    forced: &Forced,
    retries_used: u8,
    report: &AttemptReport,
    rules: &RuleSet,
) -> Evaluation {
    if let Some(failure) = failure(report, retries_used, rules) {
        return failure.into_evaluation(forced.refusals.clone());
    }
    let mut reserves = Vec::new();
    if forced.option.method.is_host_write() {
        reserves.push(ReserveCode::HostWriteCoverage);
    }
    Evaluation::NonCompliant(NonCompliant {
        executed: forced.option,
        refusals: forced.refusals.clone(),
        reserves,
        justification: forced.justification.clone(),
    })
}

/// A failed attempt: run again, or refuse with a post-operation code.
enum Failure {
    Retry,
    Refused(RefusalCode),
}

impl Failure {
    fn into_evaluation(self, mut reasons: Vec<RefusalCode>) -> Evaluation {
        match self {
            Self::Retry => Evaluation::Retry,
            Self::Refused(code) => {
                reasons.push(code);
                reasons.sort();
                Evaluation::Refused { reasons }
            }
        }
    }
}

/// Checks shared by every attempt, forced or not.
fn failure(report: &AttemptReport, retries_used: u8, rules: &RuleSet) -> Option<Failure> {
    // N8: unreadable samples, or sectors still pending or uncorrectable after
    // the operation. A retry would not make the result provable.
    let above_zero = |value: Option<u64>| value.is_some_and(|count| count > 0);
    let unreadable = report.validation == Validation::ReadErrors
        || report.smart_after.is_some_and(|smart| {
            above_zero(smart.pending_sectors) || above_zero(smart.offline_uncorrectable)
        });
    if unreadable {
        return Some(Failure::Refused(RefusalCode::ValidationReadErrors));
    }

    // N10: verification or validation failed once retries are exhausted.
    if !report.verification_passed || report.validation == Validation::Mismatch {
        if retries_used < rules.max_retries {
            return Some(Failure::Retry);
        }
        return Some(Failure::Refused(RefusalCode::VerificationFailed));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::decide;
    use crate::method::Category;
    use crate::profile::{
        AtaProfile, AtaSanitizeCaps, AtaSecurity, DeviceIdentity, NvmeProfile, NvmeSanitizeCaps,
        ProtectionFacts, QuirkFlags,
    };
    use crate::verdict::Verdict;

    fn smart(reallocated: u64) -> AtaSmart {
        AtaSmart {
            failing: false,
            reallocated_sectors: Some(reallocated),
            reported_uncorrectable: Some(0),
            pending_sectors: Some(0),
            offline_uncorrectable: Some(0),
        }
    }

    fn hdd(smart: AtaSmart) -> DeviceProfile {
        DeviceProfile {
            identity: DeviceIdentity::default(),
            protection: ProtectionFacts::default(),
            inspection: Inspection::Ata(AtaProfile {
                solid_state: false,
                sanitize: AtaSanitizeCaps {
                    overwrite: true,
                    ..AtaSanitizeCaps::default()
                },
                security: AtaSecurity::default(),
                smart,
                ..AtaProfile::default()
            }),
            failed_unblock_attempts: 0,
            quirks: QuirkFlags::default(),
        }
    }

    fn nvme() -> DeviceProfile {
        DeviceProfile {
            inspection: Inspection::Nvme(NvmeProfile {
                sanitize: NvmeSanitizeCaps {
                    block_erase: true,
                    crypto_erase: false,
                },
                ..NvmeProfile::default()
            }),
            ..hdd(smart(0))
        }
    }

    fn ready_for(profile: &DeviceProfile) -> Ready {
        match decide(profile, &RuleSet::baseline()) {
            Verdict::Ready(ready) => ready,
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    fn passed(smart_after: Option<AtaSmart>) -> AttemptReport {
        AttemptReport {
            verification_passed: true,
            validation: Validation::Passed,
            smart_after,
        }
    }

    fn evaluate(
        profile: &DeviceProfile,
        executed: Method,
        retries_used: u8,
        report: AttemptReport,
    ) -> Evaluation {
        let ready = ready_for(profile);
        evaluate_attempt(
            profile,
            &ready,
            executed,
            retries_used,
            &report,
            &RuleSet::baseline(),
        )
        .expect("method allowed by the verdict")
    }

    fn compliant(evaluation: Evaluation) -> Compliant {
        match evaluation {
            Evaluation::Compliant(compliant) => compliant,
            other => panic!("expected Compliant, got {other:?}"),
        }
    }

    #[test]
    fn successful_firmware_erasure_is_compliant() {
        let compliant = compliant(evaluate(
            &nvme(),
            Method::NvmeSanitizeBlockErase,
            0,
            passed(None),
        ));
        assert_eq!(
            compliant.executed,
            MethodOption {
                method: Method::NvmeSanitizeBlockErase,
                category: Category::Purge
            }
        );
        assert_eq!(compliant.reserves, []);
        assert_eq!(compliant.disposition, Disposition::Reuse);
    }

    #[test]
    fn chosen_host_write_certifies_r7() {
        let drive = hdd(smart(0));
        let compliant = compliant(evaluate(
            &drive,
            Method::HostOverwrite,
            0,
            passed(Some(smart(0))),
        ));
        assert_eq!(compliant.executed.category, Category::Clear);
        assert_eq!(compliant.reserves, [ReserveCode::HostWriteCoverage]);
    }

    #[test]
    fn method_outside_the_verdict_is_rejected() {
        let drive = nvme();
        let ready = ready_for(&drive);
        assert_eq!(
            evaluate_attempt(
                &drive,
                &ready,
                Method::NvmeFormatCryptoErase,
                0,
                &passed(None),
                &RuleSet::baseline(),
            ),
            Err(EvaluationError::MethodNotAllowed(
                Method::NvmeFormatCryptoErase
            ))
        );
    }

    #[test]
    fn n8_validation_read_errors_refuse_without_retry() {
        let report = AttemptReport {
            validation: Validation::ReadErrors,
            ..passed(None)
        };
        assert_eq!(
            evaluate(&nvme(), Method::NvmeSanitizeBlockErase, 0, report),
            Evaluation::Refused {
                reasons: vec![RefusalCode::ValidationReadErrors]
            }
        );
    }

    #[test]
    fn n8_sectors_still_pending_after_operation() {
        for after in [
            AtaSmart {
                pending_sectors: Some(2),
                ..smart(0)
            },
            AtaSmart {
                offline_uncorrectable: Some(1),
                ..smart(0)
            },
        ] {
            assert_eq!(
                evaluate(
                    &hdd(smart(0)),
                    Method::AtaSanitizeOverwrite,
                    0,
                    passed(Some(after))
                ),
                Evaluation::Refused {
                    reasons: vec![RefusalCode::ValidationReadErrors]
                }
            );
        }
    }

    #[test]
    fn failed_check_is_retried_while_retries_remain() {
        for report in [
            AttemptReport {
                verification_passed: false,
                ..passed(None)
            },
            AttemptReport {
                validation: Validation::Mismatch,
                ..passed(None)
            },
        ] {
            assert_eq!(
                evaluate(&nvme(), Method::NvmeSanitizeBlockErase, 0, report),
                Evaluation::Retry
            );
        }
    }

    #[test]
    fn n10_failed_check_after_retries_refuses() {
        let report = AttemptReport {
            validation: Validation::Mismatch,
            ..passed(None)
        };
        assert_eq!(
            evaluate(&nvme(), Method::NvmeSanitizeBlockErase, 1, report),
            Evaluation::Refused {
                reasons: vec![RefusalCode::VerificationFailed]
            }
        );
    }

    #[test]
    fn n10_retry_count_comes_from_the_rule_set() {
        let drive = nvme();
        let ready = ready_for(&drive);
        let rules = RuleSet {
            max_retries: 2,
            ..RuleSet::baseline()
        };
        let report = AttemptReport {
            verification_passed: false,
            ..passed(None)
        };
        let evaluate = |retries_used| {
            evaluate_attempt(
                &drive,
                &ready,
                Method::NvmeSanitizeBlockErase,
                retries_used,
                &report,
                &rules,
            )
        };
        assert_eq!(evaluate(1), Ok(Evaluation::Retry));
        assert_eq!(
            evaluate(2),
            Ok(Evaluation::Refused {
                reasons: vec![RefusalCode::VerificationFailed]
            })
        );
    }

    #[test]
    fn d5_growing_reallocation_recommends_destruction() {
        let compliant = compliant(evaluate(
            &hdd(smart(3)),
            Method::AtaSanitizeOverwrite,
            0,
            passed(Some(smart(4))),
        ));
        assert_eq!(
            compliant.disposition,
            Disposition::DestroyRecommended {
                reasons: vec![DispositionCode::ReallocationGrowing]
            }
        );
        assert_eq!(compliant.reserves, [ReserveCode::ReallocatedSectors]);
    }

    #[test]
    fn d5_adds_to_existing_dispositions() {
        let before = AtaSmart {
            failing: true,
            ..smart(60)
        };
        let compliant = compliant(evaluate(
            &hdd(before),
            Method::AtaSanitizeOverwrite,
            0,
            passed(Some(smart(61))),
        ));
        assert_eq!(
            compliant.disposition,
            Disposition::DestroyRecommended {
                reasons: vec![
                    DispositionCode::SmartFailing,
                    DispositionCode::ReallocatedAboveThreshold,
                    DispositionCode::ReallocationGrowing,
                ]
            }
        );
    }

    fn forced() -> Forced {
        crate::force::force(
            &Verdict::say_no(vec![RefusalCode::InspectionFailed]),
            "Inspection impossible, écriture forcée",
            &RuleSet::baseline(),
        )
        .expect("N1 is forcible")
    }

    #[test]
    fn forced_erasure_that_passes_is_non_compliant_never_compliant() {
        let evaluation = evaluate_forced_attempt(&forced(), 0, &passed(None), &RuleSet::baseline());
        assert_eq!(
            evaluation,
            Evaluation::NonCompliant(NonCompliant {
                executed: MethodOption {
                    method: Method::HostOverwrite,
                    category: Category::Clear
                },
                refusals: vec![RefusalCode::InspectionFailed],
                reserves: vec![ReserveCode::HostWriteCoverage],
                justification: "Inspection impossible, écriture forcée".to_owned(),
            })
        );
    }

    #[test]
    fn forced_erasure_follows_the_same_retry_rules() {
        let report = AttemptReport {
            validation: Validation::Mismatch,
            ..passed(None)
        };
        let rules = RuleSet::baseline();
        assert_eq!(
            evaluate_forced_attempt(&forced(), 0, &report, &rules),
            Evaluation::Retry
        );
        assert_eq!(
            evaluate_forced_attempt(&forced(), 1, &report, &rules),
            Evaluation::Refused {
                reasons: vec![
                    RefusalCode::InspectionFailed,
                    RefusalCode::VerificationFailed
                ]
            }
        );
    }

    #[test]
    fn forced_erasure_with_read_errors_is_refused() {
        let report = AttemptReport {
            validation: Validation::ReadErrors,
            ..passed(None)
        };
        assert_eq!(
            evaluate_forced_attempt(&forced(), 0, &report, &RuleSet::baseline()),
            Evaluation::Refused {
                reasons: vec![
                    RefusalCode::InspectionFailed,
                    RefusalCode::ValidationReadErrors
                ]
            }
        );
    }

    #[test]
    fn stable_reallocation_keeps_the_drive_reusable() {
        let compliant = compliant(evaluate(
            &hdd(smart(3)),
            Method::AtaSanitizeOverwrite,
            0,
            passed(Some(smart(3))),
        ));
        assert_eq!(compliant.disposition, Disposition::Reuse);
    }
}
