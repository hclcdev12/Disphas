//! Verification of a batch certificate: the attestations and journals of a
//! batch, attached to one PDF as `<report>.cose` and `<report>.json`.
//!
//! Each drive is verified on its own, its journal checked, its file name
//! matched with its report identifier, and every drive must name the same
//! batch. The batch is authentic only if every drive is.

use ed25519_dalek::VerifyingKey;

use crate::display::uuid;
use crate::model::Batch;
use crate::verify::{Revocation, Verified, VerifyError, verify};

/// Why one drive of a batch is not accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DriveError {
    /// The attestation does not verify.
    #[error(transparent)]
    Verify(VerifyError),
    /// The file name is not the report identifier the attestation carries.
    #[error("file name does not match the report identifier")]
    NameMismatch,
    /// The attestation carries no batch.
    #[error("not erased in a batch")]
    NoBatch,
    /// The attestation names another batch than the first drive.
    #[error("belongs to another batch")]
    OtherBatch,
}

/// One drive of a batch.
#[derive(Debug)]
pub struct Drive {
    /// Report identifier, from the file name.
    pub report: String,
    /// The verified attestation, or why it is not accepted.
    pub outcome: Result<Verified, DriveError>,
    /// Journal check: `None` when no journal is attached.
    pub journal_matches: Option<bool>,
}

impl Drive {
    /// Authentic, with a matching journal.
    pub fn accepted(&self) -> bool {
        self.outcome.is_ok() && self.journal_matches == Some(true)
    }
}

/// The verification of a batch.
#[derive(Debug)]
pub struct BatchVerification {
    /// The batch named by the first authentic drive.
    pub batch: Option<Batch>,
    /// Every drive, in report order.
    pub drives: Vec<Drive>,
}

impl BatchVerification {
    /// Every drive accepted, and at least one drive.
    pub fn authentic(&self) -> bool {
        !self.drives.is_empty() && self.drives.iter().all(Drive::accepted)
    }
}

/// Whether `files` (name, content) look like a batch: report attestations
/// named `<uuid>.cose`, and no single `attestation.cose`.
pub fn is_batch(files: &[(String, Vec<u8>)]) -> bool {
    !files.iter().any(|(name, _)| name == "attestation.cose")
        && files
            .iter()
            .any(|(name, _)| name.strip_suffix(".cose").is_some_and(is_report_id))
}

fn is_report_id(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit() && !c.is_ascii_uppercase(),
        })
}

/// Verifies every `<report>.cose` of `files` against `root` and
/// `revocations`, with its `<report>.json`.
pub fn verify_batch(
    files: &[(String, Vec<u8>)],
    root: &VerifyingKey,
    revocations: &[Revocation],
) -> BatchVerification {
    let mut reports: Vec<&str> = files
        .iter()
        .filter_map(|(name, _)| name.strip_suffix(".cose"))
        .filter(|stem| is_report_id(stem))
        .collect();
    reports.sort_unstable();
    reports.dedup();

    let mut batch: Option<Batch> = None;
    let mut drives = Vec::with_capacity(reports.len());
    for report in reports {
        let find = |extension: &str| {
            files
                .iter()
                .find(|(name, _)| *name == format!("{report}.{extension}"))
                .map(|(_, content)| content.as_slice())
        };
        let Some(signed) = find("cose") else {
            continue;
        };
        let journal = find("json");
        let outcome = verify(signed, root, revocations)
            .map_err(DriveError::Verify)
            .and_then(|verified| {
                if uuid(&verified.attestation.id) != report {
                    return Err(DriveError::NameMismatch);
                }
                match (&verified.attestation.batch, &batch) {
                    (None, _) => Err(DriveError::NoBatch),
                    (Some(own), Some(first)) if own != first => Err(DriveError::OtherBatch),
                    (Some(own), None) => {
                        batch = Some(own.clone());
                        Ok(verified)
                    }
                    (Some(_), Some(_)) => Ok(verified),
                }
            });
        let journal_matches = match (&outcome, journal) {
            (Ok(verified), Some(journal)) => Some(verified.matches_journal(journal)),
            (_, None) => None,
            (Err(_), Some(_)) => Some(false),
        };
        drives.push(Drive {
            report: report.to_owned(),
            outcome,
            journal_matches,
        });
    }
    BatchVerification { batch, drives }
}
