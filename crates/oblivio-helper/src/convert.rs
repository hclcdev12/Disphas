//! Conversion from `oblivio-core` types to their wire form.
//!
//! Every match is exhaustive on purpose: a code added to the engine without
//! its wire counterpart breaks the build instead of being silently dropped.

use oblivio_core::codes as engine;
use oblivio_core::method::{Category, Method, MethodOption};
use oblivio_core::profile::{DeviceIdentity, Inspection};
use oblivio_core::verdict::{Disposition, Ready, Verdict};
use oblivio_protocol::codes as wire;
use oblivio_protocol::command as wire_command;
use oblivio_protocol::response::DriveKind;
use oblivio_protocol::verdict as wire_verdict;

/// Wire form of a protection code.
pub fn protect_code(code: engine::ProtectCode) -> wire::ProtectCode {
    use engine::ProtectCode as E;
    use wire::ProtectCode as W;
    match code {
        E::SystemDisk => W::SystemDisk,
        E::MountedPartition => W::MountedPartition,
        E::ActiveSwap => W::ActiveSwap,
        E::VolumeMember => W::VolumeMember,
        E::SiblingNamespaceProtected => W::SiblingNamespaceProtected,
    }
}

/// Wire form of a blocking code.
pub fn block_code(code: engine::BlockCode) -> wire::BlockCode {
    use engine::BlockCode as E;
    use wire::BlockCode as W;
    match code {
        E::SecurityFrozen => W::SecurityFrozen,
        E::SecurityLocked => W::SecurityLocked,
        E::SanitizeFrozen => W::SanitizeFrozen,
        E::SanitizeInProgress => W::SanitizeInProgress,
        E::SanitizeFailed => W::SanitizeFailed,
        E::PasswordAttemptsExpired => W::PasswordAttemptsExpired,
        E::SecurityEnabled => W::SecurityEnabled,
    }
}

/// Wire form of a refusal code.
pub fn refusal_code(code: engine::RefusalCode) -> wire::RefusalCode {
    use engine::RefusalCode as E;
    use wire::RefusalCode as W;
    match code {
        E::InspectionFailed => W::InspectionFailed,
        E::ReadOnlyMedia => W::ReadOnlyMedia,
        E::ValidationReadErrors => W::ValidationReadErrors,
        E::NoMethodAvailable => W::NoMethodAvailable,
        E::VerificationFailed => W::VerificationFailed,
    }
}

/// Wire form of a disposition code.
pub fn disposition_code(code: engine::DispositionCode) -> wire::DispositionCode {
    use engine::DispositionCode as E;
    use wire::DispositionCode as W;
    match code {
        E::ReliabilityDegraded => W::ReliabilityDegraded,
        E::VolatileBackupFailed => W::VolatileBackupFailed,
        E::SmartFailing => W::SmartFailing,
        E::ReallocatedAboveThreshold => W::ReallocatedAboveThreshold,
        E::ReallocationGrowing => W::ReallocationGrowing,
    }
}

/// Wire form of a reserve code.
pub fn reserve_code(code: engine::ReserveCode) -> wire::ReserveCode {
    use engine::ReserveCode as E;
    use wire::ReserveCode as W;
    match code {
        E::SpareBelowThreshold => W::SpareBelowThreshold,
        E::EnduranceExhausted => W::EnduranceExhausted,
        E::MediaErrors => W::MediaErrors,
        E::ReallocatedSectors => W::ReallocatedSectors,
        E::PendingOrUncorrectable => W::PendingOrUncorrectable,
        E::ReportedUncorrectable => W::ReportedUncorrectable,
        E::HostWriteCoverage => W::HostWriteCoverage,
        E::KnownQuirk => W::KnownQuirk,
    }
}

/// Wire form of a category.
pub fn category(category: Category) -> wire::Category {
    match category {
        Category::Clear => wire::Category::Clear,
        Category::Purge => wire::Category::Purge,
    }
}

/// Wire form of a method.
pub fn method(method: Method) -> wire::Method {
    use wire::Method as W;
    match method {
        Method::NvmeSanitizeBlockErase => W::NvmeSanitizeBlockErase,
        Method::NvmeSanitizeCryptoErase => W::NvmeSanitizeCryptoErase,
        Method::NvmeFormatCryptoErase => W::NvmeFormatCryptoErase,
        Method::NvmeFormatUserDataErase => W::NvmeFormatUserDataErase,
        Method::AtaSanitizeBlockErase => W::AtaSanitizeBlockErase,
        Method::AtaSanitizeCryptoScramble => W::AtaSanitizeCryptoScramble,
        Method::AtaSanitizeOverwrite => W::AtaSanitizeOverwrite,
        Method::AtaSecurityEraseEnhanced => W::AtaSecurityEraseEnhanced,
        Method::AtaSecurityEraseNormal => W::AtaSecurityEraseNormal,
        Method::HostOverwrite => W::HostOverwrite,
    }
}

/// Whether `method` erases every namespace of an NVMe controller (`NS_ALL`).
pub fn erases_all_namespaces(method: Method) -> bool {
    matches!(
        method,
        Method::NvmeSanitizeBlockErase | Method::NvmeSanitizeCryptoErase
    )
}

/// Wire form of a drive identity. The firmware revision is not part of it.
pub fn identity(identity: &DeviceIdentity) -> wire_command::DeviceIdentity {
    wire_command::DeviceIdentity {
        model: identity.model.clone(),
        serial: identity.serial.clone(),
        wwn: identity.wwn.clone(),
        eui64: identity.eui64.clone(),
        nguid: identity.nguid.clone(),
    }
}

/// Kind of drive, unknown when the inspection failed.
pub fn drive_kind(inspection: &Inspection) -> Option<DriveKind> {
    match inspection {
        Inspection::Failed => None,
        Inspection::Nvme(_) => Some(DriveKind::Nvme),
        Inspection::Ata(ata) if ata.solid_state => Some(DriveKind::SataSsd),
        Inspection::Ata(_) => Some(DriveKind::SataHdd),
    }
}

/// Wire form of a disposition.
pub fn disposition(disposition: &Disposition) -> wire_verdict::Disposition {
    match disposition {
        Disposition::Reuse => wire_verdict::Disposition::Reuse {},
        Disposition::DestroyRecommended { reasons } => {
            wire_verdict::Disposition::DestroyRecommended {
                reasons: reasons.iter().copied().map(disposition_code).collect(),
            }
        }
    }
}

fn choice(option: MethodOption, reserves: &[engine::ReserveCode]) -> wire_verdict::MethodChoice {
    wire_verdict::MethodChoice {
        method: method(option.method),
        category: category(option.category),
        reserves: reserves.iter().copied().map(reserve_code).collect(),
        all_namespaces: erases_all_namespaces(option.method),
    }
}

fn ready_choice(ready: &Ready, option: MethodOption) -> wire_verdict::MethodChoice {
    choice(option, &ready.method_reserves(option.method))
}

/// Wire form of a verdict.
pub fn verdict(verdict: &Verdict) -> wire_verdict::Verdict {
    match verdict {
        Verdict::Protected { reasons } => wire_verdict::Verdict::Protected {
            reasons: reasons.iter().copied().map(protect_code).collect(),
        },
        Verdict::Blocked { reasons } => wire_verdict::Verdict::Blocked {
            reasons: reasons.iter().copied().map(block_code).collect(),
        },
        Verdict::SayNo { reasons, forcible } => wire_verdict::Verdict::SayNo {
            reasons: reasons.iter().copied().map(refusal_code).collect(),
            forcible: forcible.map(|option| {
                let reserves: &[engine::ReserveCode] = if option.method.is_host_write() {
                    &[engine::ReserveCode::HostWriteCoverage]
                } else {
                    &[]
                };
                choice(option, reserves)
            }),
        },
        Verdict::Ready(ready) => wire_verdict::Verdict::Ready {
            recommended: ready_choice(ready, ready.recommended),
            alternatives: ready
                .alternatives
                .iter()
                .map(|option| ready_choice(ready, *option))
                .collect(),
            disposition: disposition(&ready.disposition),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_keep_their_identifier_on_the_wire() {
        for &code in engine::ProtectCode::ALL {
            assert_eq!(protect_code(code).as_str(), code.as_str());
        }
        for &code in engine::BlockCode::ALL {
            assert_eq!(block_code(code).as_str(), code.as_str());
        }
        for &code in engine::RefusalCode::ALL {
            assert_eq!(refusal_code(code).as_str(), code.as_str());
        }
        for &code in engine::DispositionCode::ALL {
            assert_eq!(disposition_code(code).as_str(), code.as_str());
        }
        for &code in engine::ReserveCode::ALL {
            assert_eq!(reserve_code(code).as_str(), code.as_str());
        }
    }

    #[test]
    fn wire_has_no_code_the_engine_lacks() {
        assert_eq!(wire::ProtectCode::ALL.len(), engine::ProtectCode::ALL.len());
        assert_eq!(wire::BlockCode::ALL.len(), engine::BlockCode::ALL.len());
        assert_eq!(wire::RefusalCode::ALL.len(), engine::RefusalCode::ALL.len());
        assert_eq!(
            wire::DispositionCode::ALL.len(),
            engine::DispositionCode::ALL.len()
        );
        assert_eq!(wire::ReserveCode::ALL.len(), engine::ReserveCode::ALL.len());
    }

    #[test]
    fn forcible_host_write_carries_r7() {
        let verdict = verdict(&Verdict::say_no(vec![
            engine::RefusalCode::InspectionFailed,
        ]));
        let wire_verdict::Verdict::SayNo {
            forcible: Some(forcible),
            ..
        } = verdict
        else {
            panic!("N1 must be forcible");
        };
        assert_eq!(forcible.reserves, [wire::ReserveCode::HostWriteCoverage]);
        assert_eq!(forcible.category, wire::Category::Clear);
    }
}
