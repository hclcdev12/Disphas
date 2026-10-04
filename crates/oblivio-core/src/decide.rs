//! The decision engine, steps 1 to 4 of `docs/moteur_decision.md`.
//!
//! Deterministic: the same profile and the same rule set always give the same
//! verdict. Each step short-circuits: the first non-green state wins.

use crate::codes::{BlockCode, DispositionCode, ProtectCode, RefusalCode, ReserveCode};
use crate::method::{Category, Method, MethodOption};
use crate::profile::{
    CriticalWarning, DeviceProfile, Inspection, ProtectionFacts, QuirkFlags, SanitizeState,
};
use crate::rules::RuleSet;
use crate::verdict::{Disposition, Ready, Verdict};

/// Returns the single verdict for `profile` under `rules`.
pub fn decide(profile: &DeviceProfile, rules: &RuleSet) -> Verdict {
    // 1. Protections.
    let protections = protections(&profile.protection);
    if !protections.is_empty() {
        return Verdict::Protected {
            reasons: protections,
        };
    }
    if profile.inspection == Inspection::Failed {
        return Verdict::say_no(vec![RefusalCode::InspectionFailed]);
    }

    // 2. Blocking conditions. A condition only blocks when it stands between
    // the operator and the preferred method; once the operator has failed to
    // resolve it too many times, the engine falls back to what remains.
    let candidates = candidates(&profile.inspection, profile.quirks, rules);
    let restrictions = Restrictions::of(&profile.inspection);
    let blocks = restrictions.blocking(candidates.first().map(|option| option.method));
    if !blocks.is_empty() && profile.failed_unblock_attempts < rules.max_unblock_attempts {
        return Verdict::Blocked { reasons: blocks };
    }

    // 3. Health: refusals only. Dispositions and reserves never stop a READY drive.
    let refusals = refusals(&profile.inspection);
    if !refusals.is_empty() {
        return Verdict::say_no(refusals);
    }

    // 4. Method.
    let mut available = candidates
        .into_iter()
        .filter(|option| restrictions.allows(option.method));
    let Some(recommended) = available.next() else {
        return Verdict::say_no(vec![RefusalCode::NoMethodAvailable]);
    };
    Verdict::Ready(Ready {
        recommended,
        alternatives: available.collect(),
        drive_reserves: drive_reserves(profile, rules),
        disposition: disposition(&profile.inspection, rules),
        format_erases_all_namespaces: matches!(
            &profile.inspection,
            Inspection::Nvme(nvme) if nvme.format.erases_all_namespaces
        ),
    })
}

fn protections(facts: &ProtectionFacts) -> Vec<ProtectCode> {
    [
        (facts.hosts_system, ProtectCode::SystemDisk),
        (facts.has_mounted_partition, ProtectCode::MountedPartition),
        (facts.has_active_swap, ProtectCode::ActiveSwap),
        (facts.is_volume_member, ProtectCode::VolumeMember),
        (
            facts.sibling_namespace_protected,
            ProtectCode::SiblingNamespaceProtected,
        ),
    ]
    .into_iter()
    .filter_map(|(applies, code)| applies.then_some(code))
    .collect()
}

/// What the current drive state forbids.
#[derive(Debug, Default)]
struct Restrictions {
    /// Conditions that make every method unavailable.
    all_methods: Vec<BlockCode>,
    /// B1: only ATA Security Erase is unavailable.
    security_frozen: bool,
    /// B7: only ATA Security Erase is unavailable: we never override a
    /// password someone else set.
    security_enabled: bool,
    /// B3: only ATA Sanitize is unavailable.
    sanitize_frozen: bool,
}

impl Restrictions {
    fn of(inspection: &Inspection) -> Self {
        let mut restrictions = Self::default();
        let sanitize_state = match inspection {
            Inspection::Failed => return restrictions,
            Inspection::Nvme(nvme) => nvme.sanitize_state,
            Inspection::Ata(ata) => {
                restrictions.security_frozen = ata.security.frozen;
                restrictions.security_enabled = ata.security.enabled && !ata.security.locked;
                restrictions.sanitize_frozen = ata.sanitize_frozen;
                if ata.security.locked {
                    restrictions.all_methods.push(BlockCode::SecurityLocked);
                }
                if ata.security.attempts_expired {
                    restrictions
                        .all_methods
                        .push(BlockCode::PasswordAttemptsExpired);
                }
                ata.sanitize_state
            }
        };
        match sanitize_state {
            SanitizeState::Idle => {}
            SanitizeState::InProgress => {
                restrictions.all_methods.push(BlockCode::SanitizeInProgress)
            }
            SanitizeState::Failed => restrictions.all_methods.push(BlockCode::SanitizeFailed),
        }
        restrictions
    }

    fn allows(&self, method: Method) -> bool {
        self.all_methods.is_empty()
            && !((self.security_frozen || self.security_enabled) && method.is_ata_security_erase())
            && !(self.sanitize_frozen && method.is_ata_sanitize())
    }

    /// Conditions standing between the operator and `preferred`, sorted by code.
    fn blocking(&self, preferred: Option<Method>) -> Vec<BlockCode> {
        let mut codes = self.all_methods.clone();
        if self.security_frozen && preferred.is_some_and(Method::is_ata_security_erase) {
            codes.push(BlockCode::SecurityFrozen);
        }
        if self.security_enabled && preferred.is_some_and(Method::is_ata_security_erase) {
            codes.push(BlockCode::SecurityEnabled);
        }
        if self.sanitize_frozen && preferred.is_some_and(Method::is_ata_sanitize) {
            codes.push(BlockCode::SanitizeFrozen);
        }
        codes.sort();
        codes
    }
}

/// Methods the drive supports, in order of preference (§4), ignoring its current state.
fn candidates(inspection: &Inspection, quirks: QuirkFlags, rules: &RuleSet) -> Vec<MethodOption> {
    let categories = &rules.categories;
    let mut options = Vec::new();
    let mut offer = |supported: bool, method: Method, category: Category| {
        if supported {
            options.push(MethodOption { method, category });
        }
    };
    match inspection {
        Inspection::Failed => {}
        Inspection::Nvme(nvme) => {
            offer(
                nvme.sanitize.block_erase,
                Method::NvmeSanitizeBlockErase,
                Category::Purge,
            );
            offer(
                nvme.sanitize.crypto_erase,
                Method::NvmeSanitizeCryptoErase,
                Category::Purge,
            );
            offer(
                nvme.format.supported && nvme.format.crypto_erase,
                Method::NvmeFormatCryptoErase,
                categories.nvme_format_crypto_erase,
            );
            offer(
                nvme.format.supported,
                Method::NvmeFormatUserDataErase,
                categories.nvme_format_user_data_erase,
            );
        }
        Inspection::Ata(ata) if ata.solid_state => {
            offer(
                ata.sanitize.block_erase,
                Method::AtaSanitizeBlockErase,
                Category::Purge,
            );
            offer(
                ata.sanitize.crypto_scramble,
                Method::AtaSanitizeCryptoScramble,
                Category::Purge,
            );
            let security_erase = if quirks.security_erase_purge_validated {
                Category::Purge
            } else {
                categories.ata_security_erase_ssd
            };
            offer(
                ata.security.supported && ata.security.enhanced_erase,
                Method::AtaSecurityEraseEnhanced,
                security_erase,
            );
        }
        Inspection::Ata(ata) => {
            offer(
                ata.sanitize.overwrite,
                Method::AtaSanitizeOverwrite,
                Category::Purge,
            );
            let security_erase = if ata.security.enhanced_erase {
                Method::AtaSecurityEraseEnhanced
            } else {
                Method::AtaSecurityEraseNormal
            };
            offer(
                ata.security.supported,
                security_erase,
                categories.ata_security_erase_hdd,
            );
        }
    }
    // A host write is never a Purge (invariant rule 1), whatever the rule set says.
    offer(
        inspection != &Inspection::Failed,
        Method::HostOverwrite,
        Category::Clear,
    );
    options
}

fn refusals(inspection: &Inspection) -> Vec<RefusalCode> {
    match inspection {
        Inspection::Nvme(nvme) if nvme.health.critical_warning.has(CriticalWarning::READ_ONLY) => {
            vec![RefusalCode::ReadOnlyMedia]
        }
        _ => Vec::new(),
    }
}

fn disposition(inspection: &Inspection, rules: &RuleSet) -> Disposition {
    let reasons: Vec<DispositionCode> = match inspection {
        Inspection::Failed => Vec::new(),
        Inspection::Nvme(nvme) => {
            let warning = nvme.health.critical_warning;
            [
                (
                    warning.has(CriticalWarning::RELIABILITY_DEGRADED),
                    DispositionCode::ReliabilityDegraded,
                ),
                (
                    warning.has(CriticalWarning::VOLATILE_BACKUP_FAILED),
                    DispositionCode::VolatileBackupFailed,
                ),
            ]
            .into_iter()
            .filter_map(|(applies, code)| applies.then_some(code))
            .collect()
        }
        Inspection::Ata(ata) => [
            (ata.smart.failing, DispositionCode::SmartFailing),
            (
                ata.smart
                    .reallocated_sectors
                    .is_some_and(|count| count >= rules.max_reallocated_sectors),
                DispositionCode::ReallocatedAboveThreshold,
            ),
        ]
        .into_iter()
        .filter_map(|(applies, code)| applies.then_some(code))
        .collect(),
    };
    if reasons.is_empty() {
        Disposition::Reuse
    } else {
        Disposition::DestroyRecommended { reasons }
    }
}

/// Reserves about the drive itself. R7 depends on the method: see `Ready::reserves_for`.
fn drive_reserves(profile: &DeviceProfile, rules: &RuleSet) -> Vec<ReserveCode> {
    let above_zero = |value: Option<u64>| value.is_some_and(|count| count > 0);
    let mut reserves: Vec<(bool, ReserveCode)> = match &profile.inspection {
        Inspection::Failed => Vec::new(),
        Inspection::Nvme(nvme) => vec![
            (
                nvme.health
                    .critical_warning
                    .has(CriticalWarning::SPARE_BELOW_THRESHOLD),
                ReserveCode::SpareBelowThreshold,
            ),
            (
                nvme.health.percentage_used >= 100,
                ReserveCode::EnduranceExhausted,
            ),
            (nvme.health.media_errors > 0, ReserveCode::MediaErrors),
        ],
        Inspection::Ata(ata) => vec![
            (
                ata.smart
                    .reallocated_sectors
                    .is_some_and(|count| (1..rules.max_reallocated_sectors).contains(&count)),
                ReserveCode::ReallocatedSectors,
            ),
            (
                above_zero(ata.smart.pending_sectors)
                    || above_zero(ata.smart.offline_uncorrectable),
                ReserveCode::PendingOrUncorrectable,
            ),
            (
                above_zero(ata.smart.reported_uncorrectable),
                ReserveCode::ReportedUncorrectable,
            ),
        ],
    };
    reserves.push((profile.quirks.known_issue, ReserveCode::KnownQuirk));
    reserves
        .into_iter()
        .filter_map(|(applies, code)| applies.then_some(code))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{
        AtaProfile, AtaSanitizeCaps, AtaSecurity, AtaSmart, DeviceIdentity, NvmeFormatCaps,
        NvmeProfile, NvmeSanitizeCaps,
    };

    // --- Fixtures: fictional, healthy drives with every capability of their class.

    fn profile(inspection: Inspection) -> DeviceProfile {
        DeviceProfile {
            identity: DeviceIdentity::default(),
            protection: ProtectionFacts::default(),
            inspection,
            failed_unblock_attempts: 0,
            quirks: QuirkFlags::default(),
        }
    }

    fn nvme() -> NvmeProfile {
        NvmeProfile {
            sanitize: NvmeSanitizeCaps {
                block_erase: true,
                crypto_erase: true,
            },
            format: NvmeFormatCaps {
                supported: true,
                crypto_erase: true,
                erases_all_namespaces: false,
            },
            ..NvmeProfile::default()
        }
    }

    fn healthy_smart() -> AtaSmart {
        AtaSmart {
            failing: false,
            reallocated_sectors: Some(0),
            reported_uncorrectable: Some(0),
            pending_sectors: Some(0),
            offline_uncorrectable: Some(0),
        }
    }

    fn sata_ssd() -> AtaProfile {
        AtaProfile {
            solid_state: true,
            sanitize: AtaSanitizeCaps {
                block_erase: true,
                crypto_scramble: true,
                overwrite: false,
            },
            security: AtaSecurity {
                supported: true,
                enhanced_erase: true,
                ..AtaSecurity::default()
            },
            smart: healthy_smart(),
            ..AtaProfile::default()
        }
    }

    fn sata_ssd_security_erase_only() -> AtaProfile {
        AtaProfile {
            sanitize: AtaSanitizeCaps::default(),
            ..sata_ssd()
        }
    }

    fn hdd() -> AtaProfile {
        AtaProfile {
            solid_state: false,
            sanitize: AtaSanitizeCaps {
                overwrite: true,
                ..AtaSanitizeCaps::default()
            },
            security: AtaSecurity {
                supported: true,
                enhanced_erase: true,
                ..AtaSecurity::default()
            },
            smart: healthy_smart(),
            ..AtaProfile::default()
        }
    }

    fn nvme_with_warning(bits: u8) -> DeviceProfile {
        let mut drive = nvme();
        drive.health.critical_warning = CriticalWarning(bits);
        profile(Inspection::Nvme(drive))
    }

    fn ssd_with_smart(smart: AtaSmart) -> DeviceProfile {
        profile(Inspection::Ata(AtaProfile {
            smart,
            ..sata_ssd()
        }))
    }

    fn decide_baseline(profile: &DeviceProfile) -> Verdict {
        decide(profile, &RuleSet::baseline())
    }

    fn ready(verdict: Verdict) -> Ready {
        match verdict {
            Verdict::Ready(ready) => ready,
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    fn recommended_reserves(ready: &Ready) -> Vec<ReserveCode> {
        ready
            .reserves_for(ready.recommended.method)
            .expect("the recommended method is always allowed")
    }

    fn option(method: Method, category: Category) -> MethodOption {
        MethodOption { method, category }
    }

    fn protected(reasons: &[ProtectCode]) -> Verdict {
        Verdict::Protected {
            reasons: reasons.to_vec(),
        }
    }

    fn blocked(reasons: &[BlockCode]) -> Verdict {
        Verdict::Blocked {
            reasons: reasons.to_vec(),
        }
    }

    fn say_no(reasons: &[RefusalCode]) -> Verdict {
        Verdict::say_no(reasons.to_vec())
    }

    fn destroy(reasons: &[DispositionCode]) -> Disposition {
        Disposition::DestroyRecommended {
            reasons: reasons.to_vec(),
        }
    }

    // --- Baseline: healthy drives are READY with a Purge and no warning.

    #[test]
    fn healthy_drives_are_ready_for_reuse_without_reserves() {
        for drive in [
            Inspection::Nvme(nvme()),
            Inspection::Ata(sata_ssd()),
            Inspection::Ata(hdd()),
        ] {
            let ready = ready(decide_baseline(&profile(drive)));
            assert_eq!(ready.recommended.category, Category::Purge);
            assert_eq!(recommended_reserves(&ready), []);
            assert_eq!(ready.disposition, Disposition::Reuse);
        }
    }

    #[test]
    fn decision_is_deterministic() {
        let drive = ssd_with_smart(AtaSmart {
            reallocated_sectors: Some(7),
            ..healthy_smart()
        });
        assert_eq!(decide_baseline(&drive), decide_baseline(&drive));
    }

    // --- 1. Protections (P).

    fn with_protection(facts: ProtectionFacts) -> DeviceProfile {
        DeviceProfile {
            protection: facts,
            ..profile(Inspection::Nvme(nvme()))
        }
    }

    #[test]
    fn p1_system_disk_is_protected() {
        let drive = with_protection(ProtectionFacts {
            hosts_system: true,
            ..ProtectionFacts::default()
        });
        assert_eq!(
            decide_baseline(&drive),
            protected(&[ProtectCode::SystemDisk])
        );
    }

    #[test]
    fn p2_mounted_partition_is_protected() {
        let drive = with_protection(ProtectionFacts {
            has_mounted_partition: true,
            ..ProtectionFacts::default()
        });
        assert_eq!(
            decide_baseline(&drive),
            protected(&[ProtectCode::MountedPartition])
        );
    }

    #[test]
    fn p3_active_swap_is_protected() {
        let drive = with_protection(ProtectionFacts {
            has_active_swap: true,
            ..ProtectionFacts::default()
        });
        assert_eq!(
            decide_baseline(&drive),
            protected(&[ProtectCode::ActiveSwap])
        );
    }

    #[test]
    fn p4_volume_member_is_protected() {
        let drive = with_protection(ProtectionFacts {
            is_volume_member: true,
            ..ProtectionFacts::default()
        });
        assert_eq!(
            decide_baseline(&drive),
            protected(&[ProtectCode::VolumeMember])
        );
    }

    #[test]
    fn p5_protected_sibling_namespace_protects_the_controller() {
        let drive = with_protection(ProtectionFacts {
            sibling_namespace_protected: true,
            ..ProtectionFacts::default()
        });
        assert_eq!(
            decide_baseline(&drive),
            protected(&[ProtectCode::SiblingNamespaceProtected])
        );
    }

    #[test]
    fn every_protection_is_reported() {
        let drive = with_protection(ProtectionFacts {
            hosts_system: true,
            has_mounted_partition: true,
            has_active_swap: true,
            is_volume_member: true,
            sibling_namespace_protected: true,
        });
        assert_eq!(decide_baseline(&drive), protected(ProtectCode::ALL));
    }

    #[test]
    fn protection_wins_over_everything() {
        let drive = DeviceProfile {
            protection: ProtectionFacts {
                has_mounted_partition: true,
                ..ProtectionFacts::default()
            },
            ..profile(Inspection::Failed)
        };
        assert_eq!(
            decide_baseline(&drive),
            protected(&[ProtectCode::MountedPartition])
        );
    }

    // --- 2. Blocking conditions (B).

    fn ssd_with_security(security: AtaSecurity) -> AtaProfile {
        AtaProfile {
            security,
            ..sata_ssd()
        }
    }

    #[test]
    fn b1_frozen_blocks_when_security_erase_is_preferred() {
        let mut drive = sata_ssd_security_erase_only();
        drive.security.frozen = true;
        assert_eq!(
            decide_baseline(&profile(Inspection::Ata(drive))),
            blocked(&[BlockCode::SecurityFrozen])
        );
        assert_eq!(
            BlockCode::SecurityFrozen.guidance(),
            crate::verdict::Guidance::SuspendAndResume
        );
    }

    #[test]
    fn b1_frozen_does_not_block_when_sanitize_is_preferred() {
        let drive = ssd_with_security(AtaSecurity {
            frozen: true,
            ..sata_ssd().security
        });
        let ready = ready(decide_baseline(&profile(Inspection::Ata(drive))));
        assert_eq!(
            ready.recommended,
            option(Method::AtaSanitizeBlockErase, Category::Purge)
        );
        assert!(
            ready
                .alternatives
                .iter()
                .all(|option| !option.method.is_ata_security_erase()),
            "a frozen drive must not offer Security Erase"
        );
    }

    #[test]
    fn b2_locked_drive_is_blocked() {
        let drive = ssd_with_security(AtaSecurity {
            locked: true,
            ..sata_ssd().security
        });
        assert_eq!(
            decide_baseline(&profile(Inspection::Ata(drive))),
            blocked(&[BlockCode::SecurityLocked])
        );
    }

    #[test]
    fn b3_sanitize_freeze_lock_blocks_when_sanitize_is_preferred() {
        let drive = AtaProfile {
            sanitize_frozen: true,
            ..sata_ssd()
        };
        assert_eq!(
            decide_baseline(&profile(Inspection::Ata(drive))),
            blocked(&[BlockCode::SanitizeFrozen])
        );
    }

    #[test]
    fn b4_sanitize_in_progress_blocks() {
        let drive = NvmeProfile {
            sanitize_state: SanitizeState::InProgress,
            ..nvme()
        };
        assert_eq!(
            decide_baseline(&profile(Inspection::Nvme(drive))),
            blocked(&[BlockCode::SanitizeInProgress])
        );
    }

    #[test]
    fn b5_failed_sanitize_blocks() {
        let drive = AtaProfile {
            sanitize_state: SanitizeState::Failed,
            ..sata_ssd()
        };
        assert_eq!(
            decide_baseline(&profile(Inspection::Ata(drive))),
            blocked(&[BlockCode::SanitizeFailed])
        );
    }

    #[test]
    fn b6_expired_password_attempts_block() {
        let drive = ssd_with_security(AtaSecurity {
            locked: true,
            attempts_expired: true,
            ..sata_ssd().security
        });
        assert_eq!(
            decide_baseline(&profile(Inspection::Ata(drive))),
            blocked(&[
                BlockCode::SecurityLocked,
                BlockCode::PasswordAttemptsExpired
            ])
        );
    }

    #[test]
    fn b7_foreign_password_blocks_security_erase() {
        let mut drive = sata_ssd_security_erase_only();
        drive.security.enabled = true;
        assert_eq!(
            decide_baseline(&profile(Inspection::Ata(drive))),
            blocked(&[BlockCode::SecurityEnabled])
        );
        assert_eq!(
            BlockCode::SecurityEnabled.guidance(),
            crate::verdict::Guidance::RemovePassword
        );
    }

    #[test]
    fn b7_does_not_block_when_sanitize_is_preferred() {
        let mut drive = sata_ssd();
        drive.security.enabled = true;
        let ready = ready(decide_baseline(&profile(Inspection::Ata(drive))));
        assert_eq!(ready.recommended.method, Method::AtaSanitizeBlockErase);
        assert!(
            ready
                .alternatives
                .iter()
                .all(|o| !o.method.is_ata_security_erase())
        );
    }

    #[test]
    fn b7_does_not_apply_to_a_locked_drive() {
        let mut drive = sata_ssd_security_erase_only();
        drive.security.enabled = true;
        drive.security.locked = true;
        assert_eq!(
            decide_baseline(&profile(Inspection::Ata(drive))),
            blocked(&[BlockCode::SecurityLocked])
        );
    }

    #[test]
    fn blocked_drive_stays_blocked_below_the_attempt_threshold() {
        let mut drive = sata_ssd_security_erase_only();
        drive.security.frozen = true;
        let drive = DeviceProfile {
            failed_unblock_attempts: 2,
            ..profile(Inspection::Ata(drive))
        };
        assert_eq!(
            decide_baseline(&drive),
            blocked(&[BlockCode::SecurityFrozen])
        );
    }

    #[test]
    fn exhausted_unblock_falls_back_to_remaining_clear_method() {
        let mut drive = sata_ssd_security_erase_only();
        drive.security.frozen = true;
        let drive = DeviceProfile {
            failed_unblock_attempts: 3,
            ..profile(Inspection::Ata(drive))
        };
        let ready = ready(decide_baseline(&drive));
        assert_eq!(
            ready.recommended,
            option(Method::HostOverwrite, Category::Clear)
        );
        assert_eq!(ready.alternatives, []);
        assert_eq!(
            recommended_reserves(&ready),
            [ReserveCode::HostWriteCoverage]
        );
    }

    // --- 3. Refusals (N), pre-operation only.

    #[test]
    fn n1_failed_inspection_says_no() {
        assert_eq!(
            decide_baseline(&profile(Inspection::Failed)),
            say_no(&[RefusalCode::InspectionFailed])
        );
    }

    #[test]
    fn n3_read_only_media_says_no() {
        assert_eq!(
            decide_baseline(&nvme_with_warning(CriticalWarning::READ_ONLY)),
            say_no(&[RefusalCode::ReadOnlyMedia])
        );
    }

    #[test]
    fn n3_wins_over_dispositions() {
        let drive =
            nvme_with_warning(CriticalWarning::READ_ONLY | CriticalWarning::RELIABILITY_DEGRADED);
        assert_eq!(
            decide_baseline(&drive),
            say_no(&[RefusalCode::ReadOnlyMedia])
        );
    }

    #[test]
    fn n9_no_method_left_after_exhausted_unblock_says_no() {
        let drive = ssd_with_security(AtaSecurity {
            locked: true,
            ..sata_ssd().security
        });
        let drive = DeviceProfile {
            failed_unblock_attempts: 3,
            ..profile(Inspection::Ata(drive))
        };
        assert_eq!(
            decide_baseline(&drive),
            say_no(&[RefusalCode::NoMethodAvailable])
        );
    }

    #[test]
    fn n8_and_n10_are_post_operation_only() {
        let post: Vec<RefusalCode> = RefusalCode::ALL
            .iter()
            .copied()
            .filter(|code| code.is_post_operation())
            .collect();
        assert_eq!(
            post,
            [
                RefusalCode::ValidationReadErrors,
                RefusalCode::VerificationFailed
            ]
        );
    }

    // --- 3. Dispositions (D): READY, erasure certified, destruction recommended.

    #[test]
    fn d1_reliability_degraded_recommends_destruction() {
        let ready = ready(decide_baseline(&nvme_with_warning(
            CriticalWarning::RELIABILITY_DEGRADED,
        )));
        assert_eq!(ready.recommended.category, Category::Purge);
        assert_eq!(
            ready.disposition,
            destroy(&[DispositionCode::ReliabilityDegraded])
        );
    }

    #[test]
    fn d2_volatile_backup_failure_recommends_destruction() {
        let ready = ready(decide_baseline(&nvme_with_warning(
            CriticalWarning::VOLATILE_BACKUP_FAILED,
        )));
        assert_eq!(
            ready.disposition,
            destroy(&[DispositionCode::VolatileBackupFailed])
        );
    }

    #[test]
    fn d3_smart_failing_recommends_destruction() {
        let mut drive = hdd();
        drive.smart.failing = true;
        let ready = ready(decide_baseline(&profile(Inspection::Ata(drive))));
        assert_eq!(
            ready.recommended,
            option(Method::AtaSanitizeOverwrite, Category::Purge)
        );
        assert_eq!(ready.disposition, destroy(&[DispositionCode::SmartFailing]));
    }

    #[test]
    fn d4_reallocated_at_threshold_recommends_destruction_without_r4() {
        let ready = ready(decide_baseline(&ssd_with_smart(AtaSmart {
            reallocated_sectors: Some(50),
            ..healthy_smart()
        })));
        assert_eq!(
            ready.disposition,
            destroy(&[DispositionCode::ReallocatedAboveThreshold])
        );
        assert_eq!(recommended_reserves(&ready), []);
    }

    #[test]
    fn d4_threshold_comes_from_the_rule_set() {
        let rules = RuleSet {
            max_reallocated_sectors: 10,
            ..RuleSet::baseline()
        };
        let drive = ssd_with_smart(AtaSmart {
            reallocated_sectors: Some(10),
            ..healthy_smart()
        });
        assert_eq!(
            ready(decide(&drive, &rules)).disposition,
            destroy(&[DispositionCode::ReallocatedAboveThreshold])
        );
    }

    #[test]
    fn d5_is_post_operation_only() {
        let post: Vec<DispositionCode> = DispositionCode::ALL
            .iter()
            .copied()
            .filter(|code| code.is_post_operation())
            .collect();
        assert_eq!(post, [DispositionCode::ReallocationGrowing]);
    }

    // --- 3. Reserves (R): READY with a certified warning.

    #[test]
    fn r1_spare_below_threshold() {
        let ready = ready(decide_baseline(&nvme_with_warning(
            CriticalWarning::SPARE_BELOW_THRESHOLD,
        )));
        assert_eq!(
            recommended_reserves(&ready),
            [ReserveCode::SpareBelowThreshold]
        );
        assert_eq!(ready.disposition, Disposition::Reuse);
    }

    #[test]
    fn r2_endurance_exhausted() {
        let mut drive = nvme();
        drive.health.percentage_used = 100;
        let ready = ready(decide_baseline(&profile(Inspection::Nvme(drive))));
        assert_eq!(
            recommended_reserves(&ready),
            [ReserveCode::EnduranceExhausted]
        );
        assert_eq!(ready.disposition, Disposition::Reuse);
    }

    #[test]
    fn r3_media_errors() {
        let mut drive = nvme();
        drive.health.media_errors = 1;
        let ready = ready(decide_baseline(&profile(Inspection::Nvme(drive))));
        assert_eq!(recommended_reserves(&ready), [ReserveCode::MediaErrors]);
    }

    #[test]
    fn r4_reallocated_below_threshold() {
        for count in [1, 49] {
            let ready = ready(decide_baseline(&ssd_with_smart(AtaSmart {
                reallocated_sectors: Some(count),
                ..healthy_smart()
            })));
            assert_eq!(
                recommended_reserves(&ready),
                [ReserveCode::ReallocatedSectors]
            );
            assert_eq!(ready.disposition, Disposition::Reuse);
        }
    }

    #[test]
    fn r5_pending_or_offline_uncorrectable() {
        for smart in [
            AtaSmart {
                pending_sectors: Some(1),
                ..healthy_smart()
            },
            AtaSmart {
                offline_uncorrectable: Some(1),
                ..healthy_smart()
            },
        ] {
            let ready = ready(decide_baseline(&ssd_with_smart(smart)));
            assert_eq!(
                recommended_reserves(&ready),
                [ReserveCode::PendingOrUncorrectable]
            );
        }
    }

    #[test]
    fn r6_reported_uncorrectable() {
        let ready = ready(decide_baseline(&ssd_with_smart(AtaSmart {
            reported_uncorrectable: Some(3),
            ..healthy_smart()
        })));
        assert_eq!(
            recommended_reserves(&ready),
            [ReserveCode::ReportedUncorrectable]
        );
    }

    #[test]
    fn r7_host_write_when_no_firmware_method() {
        let drive = NvmeProfile::default();
        let ready = ready(decide_baseline(&profile(Inspection::Nvme(drive))));
        assert_eq!(
            ready.recommended,
            option(Method::HostOverwrite, Category::Clear)
        );
        assert_eq!(
            recommended_reserves(&ready),
            [ReserveCode::HostWriteCoverage]
        );
    }

    #[test]
    fn r7_follows_the_chosen_method_not_the_recommended_one() {
        let ready = ready(decide_baseline(&ssd_with_smart(AtaSmart {
            reallocated_sectors: Some(3),
            ..healthy_smart()
        })));
        assert_eq!(
            ready.reserves_for(Method::AtaSanitizeBlockErase),
            Some(vec![ReserveCode::ReallocatedSectors])
        );
        assert_eq!(
            ready.reserves_for(Method::HostOverwrite),
            Some(vec![
                ReserveCode::ReallocatedSectors,
                ReserveCode::HostWriteCoverage
            ])
        );
        assert_eq!(ready.reserves_for(Method::NvmeSanitizeBlockErase), None);
    }

    #[test]
    fn r8_known_quirk() {
        let drive = DeviceProfile {
            quirks: QuirkFlags {
                known_issue: true,
                ..QuirkFlags::default()
            },
            ..profile(Inspection::Nvme(nvme()))
        };
        assert_eq!(
            recommended_reserves(&ready(decide_baseline(&drive))),
            [ReserveCode::KnownQuirk]
        );
    }

    // --- 4. Method choice.

    #[test]
    fn nvme_offers_every_method_in_order_of_preference() {
        let ready = ready(decide_baseline(&profile(Inspection::Nvme(nvme()))));
        assert_eq!(
            ready.recommended,
            option(Method::NvmeSanitizeBlockErase, Category::Purge)
        );
        assert_eq!(
            ready.alternatives,
            [
                option(Method::NvmeSanitizeCryptoErase, Category::Purge),
                option(Method::NvmeFormatCryptoErase, Category::Purge),
                option(Method::NvmeFormatUserDataErase, Category::Clear),
                option(Method::HostOverwrite, Category::Clear),
            ]
        );
    }

    #[test]
    fn nvme_falls_back_to_crypto_erase() {
        let mut drive = nvme();
        drive.sanitize.block_erase = false;
        let ready = ready(decide_baseline(&profile(Inspection::Nvme(drive))));
        assert_eq!(
            ready.recommended,
            option(Method::NvmeSanitizeCryptoErase, Category::Purge)
        );
    }

    #[test]
    fn format_erases_every_namespace_when_the_controller_says_so() {
        let mut drive = nvme();
        let narrow = ready(decide_baseline(&profile(Inspection::Nvme(drive))));
        assert!(narrow.erases_all_namespaces(Method::NvmeSanitizeBlockErase));
        assert!(!narrow.erases_all_namespaces(Method::NvmeFormatCryptoErase));
        assert!(!narrow.erases_all_namespaces(Method::HostOverwrite));

        drive.format.erases_all_namespaces = true;
        let wide = ready(decide_baseline(&profile(Inspection::Nvme(drive))));
        assert!(wide.erases_all_namespaces(Method::NvmeFormatCryptoErase));
        assert!(wide.erases_all_namespaces(Method::NvmeFormatUserDataErase));
        assert!(!wide.erases_all_namespaces(Method::HostOverwrite));
    }

    #[test]
    fn nvme_format_categories_come_from_the_rule_set() {
        let drive = NvmeProfile {
            format: NvmeFormatCaps {
                supported: true,
                crypto_erase: false,
                erases_all_namespaces: false,
            },
            ..NvmeProfile::default()
        };
        let drive = profile(Inspection::Nvme(drive));
        assert_eq!(
            ready(decide_baseline(&drive)).recommended,
            option(Method::NvmeFormatUserDataErase, Category::Clear)
        );

        let mut rules = RuleSet::baseline();
        rules.categories.nvme_format_user_data_erase = Category::Purge;
        assert_eq!(
            ready(decide(&drive, &rules)).recommended,
            option(Method::NvmeFormatUserDataErase, Category::Purge)
        );
    }

    #[test]
    fn sata_ssd_offers_every_method_in_order_of_preference() {
        let ready = ready(decide_baseline(&profile(Inspection::Ata(sata_ssd()))));
        assert_eq!(
            ready.recommended,
            option(Method::AtaSanitizeBlockErase, Category::Purge)
        );
        assert_eq!(
            ready.alternatives,
            [
                option(Method::AtaSanitizeCryptoScramble, Category::Purge),
                option(Method::AtaSecurityEraseEnhanced, Category::Clear),
                option(Method::HostOverwrite, Category::Clear),
            ]
        );
    }

    #[test]
    fn ssd_security_erase_is_purge_only_when_validated_by_quirks() {
        let drive = profile(Inspection::Ata(sata_ssd_security_erase_only()));
        assert_eq!(
            ready(decide_baseline(&drive)).recommended,
            option(Method::AtaSecurityEraseEnhanced, Category::Clear)
        );

        let validated = DeviceProfile {
            quirks: QuirkFlags {
                security_erase_purge_validated: true,
                ..QuirkFlags::default()
            },
            ..drive
        };
        let ready = ready(decide_baseline(&validated));
        assert_eq!(
            ready.recommended,
            option(Method::AtaSecurityEraseEnhanced, Category::Purge)
        );
        assert_eq!(
            recommended_reserves(&ready),
            [],
            "a positive quirk is not a reserve"
        );
    }

    #[test]
    fn hdd_offers_every_method_in_order_of_preference() {
        let ready = ready(decide_baseline(&profile(Inspection::Ata(hdd()))));
        assert_eq!(
            ready.recommended,
            option(Method::AtaSanitizeOverwrite, Category::Purge)
        );
        assert_eq!(
            ready.alternatives,
            [
                option(Method::AtaSecurityEraseEnhanced, Category::Purge),
                option(Method::HostOverwrite, Category::Clear),
            ]
        );
    }

    #[test]
    fn hdd_uses_normal_security_erase_without_enhanced() {
        let mut drive = hdd();
        drive.sanitize.overwrite = false;
        drive.security.enhanced_erase = false;
        assert_eq!(
            ready(decide_baseline(&profile(Inspection::Ata(drive)))).recommended,
            option(Method::AtaSecurityEraseNormal, Category::Purge)
        );
    }

    #[test]
    fn host_write_is_never_purge_whatever_the_rules() {
        let mut rules = RuleSet::baseline();
        rules.categories.nvme_format_user_data_erase = Category::Purge;
        rules.categories.ata_security_erase_ssd = Category::Purge;
        for drive in [
            Inspection::Nvme(nvme()),
            Inspection::Ata(sata_ssd()),
            Inspection::Ata(hdd()),
        ] {
            let ready = ready(decide(&profile(drive), &rules));
            for option in std::iter::once(ready.recommended).chain(ready.alternatives) {
                if option.method.is_host_write() {
                    assert_eq!(option.category, Category::Clear);
                }
            }
        }
    }
}
