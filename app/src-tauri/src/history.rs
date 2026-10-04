//! History of certificates: the signed reports the helper stored in
//! `<state>/reports/` (0755, files 0644), read here as the normal user
//! (`docs/protocole_ipc.md` §10). Every entry is verified before it is shown:
//! the content of a report that does not verify is never displayed.

use std::path::{Path, PathBuf};

use oblivio_attestation::display::uuid;
use oblivio_attestation::ed25519_dalek::VerifyingKey;
use oblivio_attestation::verify;
use serde::Serialize;

/// Largest signed attestation read.
const MAX_ATTESTATION_BYTES: u64 = 1 << 20;
/// Largest journal read.
const MAX_JOURNAL_BYTES: u64 = 16 << 20;
/// Most recent reports listed.
pub const MAX_ENTRIES: usize = 5000;

/// The reports folder.
pub fn reports_dir() -> PathBuf {
    oblivio_license::state_dir().join("reports")
}

/// Outcome of the checks on one report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Check {
    /// Signature chains to a trusted root and the journal matches.
    Valid,
    /// Signature, license chain or format rejected.
    Invalid,
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

/// Whether `id` has the UUID shape Oblivio uses for report files.
pub fn is_report_id(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit() && !c.is_ascii_uppercase(),
        })
}

fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }
    std::fs::read(path).ok()
}

/// The signed attestation and journal of report `id`.
pub fn read(dir: &Path, id: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    if !is_report_id(id) {
        return None;
    }
    let signed = read_bounded(&dir.join(format!("{id}.cose")), MAX_ATTESTATION_BYTES)?;
    let journal = read_bounded(&dir.join(format!("{id}.json")), MAX_JOURNAL_BYTES)?;
    Some((signed, journal))
}

/// Checks one report against `roots`.
pub fn check(id: &str, signed: &[u8], journal: &[u8], roots: &[VerifyingKey]) -> Entry {
    let Some(verified) = roots.iter().find_map(|root| verify(signed, root, &[]).ok()) else {
        return Entry {
            id: id.to_owned(),
            check: Check::Invalid,
            summary: None,
        };
    };
    let attestation = &verified.attestation;
    if !verified.matches_journal(journal) || uuid(&attestation.id) != id {
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
        }),
    }
}

/// Reports in `dir`, newest first (identifiers are UUID v7, ordered by time).
pub fn list(dir: &Path, roots: &[VerifyingKey]) -> Vec<Entry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_suffix(".cose")?;
            is_report_id(id).then(|| id.to_owned())
        })
        .collect();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids.truncate(MAX_ENTRIES);
    ids.into_iter()
        .map(|id| match read(dir, &id) {
            Some((signed, journal)) => check(&id, &signed, &journal, roots),
            None => Entry {
                id,
                check: Check::Unreadable,
                summary: None,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oblivio_attestation::model::Status;
    use oblivio_report::fixture::{JOURNAL, root, signed};

    fn store(dir: &Path, signed: &[u8], journal: &[u8]) -> String {
        let id = uuid(
            &verify(signed, &root().verifying_key(), &[])
                .expect("fixture verifies")
                .attestation
                .id,
        );
        std::fs::write(dir.join(format!("{id}.cose")), signed).expect("cose");
        std::fs::write(dir.join(format!("{id}.json")), journal).expect("json");
        id
    }

    #[test]
    fn valid_report_is_listed_with_its_content() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let id = store(
            dir.path(),
            &signed(Status::Compliant, "Atelier XYZ"),
            JOURNAL,
        );
        let listed = list(dir.path(), &[root().verifying_key()]);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);
        assert_eq!(listed[0].check, Check::Valid);
        let summary = listed[0].summary.as_ref().expect("content");
        assert_eq!(summary.status, "CONFORME");
        assert_eq!(summary.organization.as_deref(), Some("Atelier XYZ"));
    }

    #[test]
    fn untrusted_or_tampered_reports_show_no_content() {
        let dir = tempfile::tempdir().expect("temporary directory");
        store(dir.path(), &signed(Status::Compliant, "x"), JOURNAL);
        let stranger =
            oblivio_attestation::ed25519_dalek::SigningKey::from_bytes(&[9; 32]).verifying_key();
        let listed = list(dir.path(), &[stranger]);
        assert_eq!(listed[0].check, Check::Invalid);
        assert!(listed[0].summary.is_none());

        let id = listed[0].id.clone();
        std::fs::write(dir.path().join(format!("{id}.json")), b"{}").expect("tamper");
        let listed = list(dir.path(), &[root().verifying_key()]);
        assert_eq!(listed[0].check, Check::JournalMismatch);
        assert!(listed[0].summary.is_none());
    }

    #[test]
    fn missing_journal_and_foreign_files_are_handled() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let id = store(dir.path(), &signed(Status::Refused, "x"), JOURNAL);
        std::fs::remove_file(dir.path().join(format!("{id}.json"))).expect("removed");
        std::fs::write(dir.path().join("notes.cose"), b"x").expect("foreign");
        let listed = list(dir.path(), &[root().verifying_key()]);
        assert_eq!(listed.len(), 1, "only report-shaped names");
        assert_eq!(listed[0].check, Check::Unreadable);
    }

    #[test]
    fn report_ids_are_strict() {
        assert!(is_report_id("01923456-789a-7bcd-8ef0-123456789abc"));
        assert!(!is_report_id("../../etc/passwd"));
        assert!(!is_report_id("01923456-789A-7bcd-8ef0-123456789abc"));
        assert!(!is_report_id("01923456-789a-7bcd-8ef0-123456789ab"));
        assert!(read(Path::new("/"), "../etc/passwd").is_none());
    }
}
