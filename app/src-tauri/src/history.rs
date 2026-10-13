//! History of certificates: the signed reports the helper stored in
//! `<state>/reports/` (root only), listed and read through the helper
//! (`ListReports`, `GetReport`, docs/protocole_ipc.md §10). Every entry is
//! verified here before it is shown: the content of a report that does not
//! verify is never displayed.

use base64::Engine;
use disphas_attestation::display::uuid;
use disphas_attestation::{VerifyError, verify};
use disphas_protocol::response::StoredReport;

use crate::trust::Trust;
use serde::Serialize;

/// Most recent reports listed.
pub const MAX_ENTRIES: usize = 5000;

/// Outcome of the checks on one report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Check {
    /// Signature chains to a trusted root and the journal matches.
    Valid,
    /// Signature, license chain or format rejected.
    Invalid,
    /// The license was revoked at or before the certificate date.
    Revoked,
    /// The journal next to it is not the one the attestation covers.
    JournalMismatch,
    /// Files missing, too large or unreadable.
    Unreadable,
}

/// The verified content of a report, as listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// End of the operation, UTC epoch seconds.
    pub issued_at: u64,
    /// `CONFORME`, `NON_CONFORME` or `REFUS`.
    pub status: String,
    /// `Clear`, `Purge` or `None`.
    pub category: String,
    /// Method code, if one ran.
    pub method: Option<String>,
    /// Drive model.
    pub model: String,
    /// Drive serial number.
    pub serial: String,
    /// Capacity in bytes.
    pub capacity_bytes: u64,
    /// Declarative operator.
    pub operator: Option<String>,
    /// Declarative organization.
    pub organization: Option<String>,
    /// Batch identifier, if any.
    pub batch_id: Option<String>,
    /// Batch label.
    pub batch_label: Option<String>,
}

/// One report of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Report identifier, from the file name.
    pub id: String,
    /// Result of the checks.
    pub check: Check,
    /// Content, only when `check` is `Valid`.
    pub summary: Option<Summary>,
}

/// Whether `id` has the UUID shape disphas uses for report files.
pub fn is_report_id(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit() && !c.is_ascii_uppercase(),
        })
}

/// Checks one listed report against `trust`; the journal is checked
/// through its SHA-256, computed by the helper next to the file.
pub fn check(report: &StoredReport, trust: &Trust) -> Entry {
    let id = report.id.as_str();
    let unreadable = || Entry {
        id: id.to_owned(),
        check: Check::Unreadable,
        summary: None,
    };
    let (Some(signed), Some(journal_sha256)) = (&report.attestation, &report.journal_sha256) else {
        return unreadable();
    };
    let Ok(signed) = base64::engine::general_purpose::STANDARD.decode(signed) else {
        return unreadable();
    };
    let signed = signed.as_slice();
    let mut revoked = false;
    let verified = trust.roots.iter().find_map(|root| {
        verify(signed, root, &trust.revocations)
            .map_err(|error| revoked |= error == VerifyError::LicenseRevoked)
            .ok()
    });
    let Some(verified) = verified else {
        return Entry {
            id: id.to_owned(),
            check: if revoked {
                Check::Revoked
            } else {
                Check::Invalid
            },
            summary: None,
        };
    };
    let attestation = &verified.attestation;
    let hex: String = attestation
        .journal_sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if hex != *journal_sha256 || uuid(&attestation.id) != id {
        return Entry {
            id: id.to_owned(),
            check: Check::JournalMismatch,
            summary: None,
        };
    }
    Entry {
        id: id.to_owned(),
        check: Check::Valid,
        summary: Some(Summary {
            issued_at: attestation.issued_at,
            status: attestation.status.as_str().to_owned(),
            category: attestation.sanitization.category.as_str().to_owned(),
            method: attestation
                .sanitization
                .method
                .as_ref()
                .map(|code| code.as_str().to_owned()),
            model: attestation.device.model.clone(),
            serial: attestation.device.serial.clone(),
            capacity_bytes: attestation.device.capacity_bytes,
            operator: attestation.operator.clone(),
            organization: attestation.organization.clone(),
            batch_id: attestation.batch.as_ref().map(|batch| batch.id.clone()),
            batch_label: attestation
                .batch
                .as_ref()
                .and_then(|batch| batch.label.clone()),
        }),
    }
}

/// Checks every listed report, newest first as the helper sends them.
pub fn check_all(reports: &[StoredReport], trust: &Trust) -> Vec<Entry> {
    reports
        .iter()
        .take(MAX_ENTRIES)
        .map(|report| check(report, trust))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use disphas_attestation::model::Status;
    use disphas_report::fixture::{JOURNAL, root, signed};
    use sha2::Digest;

    fn trust_in(
        roots: Vec<disphas_attestation::ed25519_dalek::VerifyingKey>,
        revocations: Vec<disphas_attestation::Revocation>,
    ) -> Trust {
        Trust {
            official: roots.first().copied(),
            roots,
            revocations,
        }
    }

    fn fixture_trust() -> Trust {
        trust_in(vec![root().verifying_key()], vec![])
    }

    /// A report as `ListReports` sends it.
    fn stored(signed: &[u8], journal: &[u8]) -> StoredReport {
        let id = uuid(
            &verify(signed, &root().verifying_key(), &[])
                .expect("fixture verifies")
                .attestation
                .id,
        );
        StoredReport {
            id,
            attestation: Some(base64::engine::general_purpose::STANDARD.encode(signed)),
            journal_sha256: Some(
                sha2::Sha256::digest(journal)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            ),
        }
    }

    #[test]
    fn report_of_a_revoked_license_is_flagged() {
        let report = stored(&signed(Status::Compliant, "x"), JOURNAL);
        let revoked = disphas_attestation::Revocation {
            license_id: "LIC-0001".to_owned(),
            revoked_at: 0,
        };
        let entry = check(
            &report,
            &trust_in(vec![root().verifying_key()], vec![revoked]),
        );
        assert_eq!(entry.check, Check::Revoked);
        assert!(entry.summary.is_none());
    }

    #[test]
    fn valid_report_is_listed_with_its_content() {
        let report = stored(&signed(Status::Compliant, "Atelier XYZ"), JOURNAL);
        let listed = check_all(std::slice::from_ref(&report), &fixture_trust());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, report.id);
        assert_eq!(listed[0].check, Check::Valid);
        let summary = listed[0].summary.as_ref().expect("content");
        assert_eq!(summary.status, "CONFORME");
        assert_eq!(summary.organization.as_deref(), Some("Atelier XYZ"));
    }

    #[test]
    fn untrusted_or_tampered_reports_show_no_content() {
        let report = stored(&signed(Status::Compliant, "x"), JOURNAL);
        let stranger =
            disphas_attestation::ed25519_dalek::SigningKey::from_bytes(&[9; 32]).verifying_key();
        let entry = check(&report, &trust_in(vec![stranger], vec![]));
        assert_eq!(entry.check, Check::Invalid);
        assert!(entry.summary.is_none());

        let mut tampered = report.clone();
        tampered.journal_sha256 = Some("00".repeat(32));
        let entry = check(&tampered, &fixture_trust());
        assert_eq!(entry.check, Check::JournalMismatch);
        assert!(entry.summary.is_none());

        let mut renamed = report;
        renamed.id = "01923456-789a-7bcd-8ef0-123456789abc".to_owned();
        assert_eq!(
            check(&renamed, &fixture_trust()).check,
            Check::JournalMismatch
        );
    }

    #[test]
    fn unreadable_reports_are_flagged() {
        let mut report = stored(&signed(Status::Refused, "x"), JOURNAL);
        report.journal_sha256 = None;
        assert_eq!(check(&report, &fixture_trust()).check, Check::Unreadable);
        report.attestation = Some("not base64 !".to_owned());
        report.journal_sha256 = Some("00".repeat(32));
        assert_eq!(check(&report, &fixture_trust()).check, Check::Unreadable);
    }

    #[test]
    fn report_ids_are_strict() {
        assert!(is_report_id("01923456-789a-7bcd-8ef0-123456789abc"));
        assert!(!is_report_id("../../etc/passwd"));
        assert!(!is_report_id("01923456-789A-7bcd-8ef0-123456789abc"));
        assert!(!is_report_id("01923456-789a-7bcd-8ef0-123456789ab"));
    }
}
