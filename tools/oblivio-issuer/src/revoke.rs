//! New revocation lists: the previous list plus one revoked license, with the
//! next sequence number. Lists only grow; a revocation cannot be undone.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use oblivio_attestation::Revocation;
use oblivio_attestation::revocation::RevocationList;

/// The list that follows `current` with `license_id` revoked at `revoked_at`.
pub fn next_list(
    current: Option<&RevocationList>,
    license_id: &str,
    revoked_at: u64,
    now: u64,
) -> Result<RevocationList, String> {
    let license_id = license_id.trim();
    if license_id.is_empty() {
        return Err("the license identifier is empty".to_owned());
    }
    let mut entries = current.map(|list| list.entries.clone()).unwrap_or_default();
    if let Some(existing) = entries.iter().find(|entry| entry.license_id == license_id) {
        return Err(format!(
            "{license_id} is already revoked (at {})",
            existing.revoked_at
        ));
    }
    entries.push(Revocation {
        license_id: license_id.to_owned(),
        revoked_at,
    });
    entries.sort_by(|a, b| a.license_id.cmp(&b.license_id));
    Ok(RevocationList {
        seq: current.map_or(1, |list| list.seq + 1),
        issued_at: now,
        entries,
    })
}

/// Whether `license_id` is in the issued-licenses register (first column).
pub fn registered(register: &Path, license_id: &str) -> std::io::Result<bool> {
    let text = std::fs::read_to_string(register)?;
    Ok(text
        .lines()
        .skip(1)
        .filter_map(|line| line.split(',').next())
        .any(|id| id.trim_matches('"') == license_id))
}

/// Writes a new list file; never overwrites one. The list is public: 0644.
pub fn write_new(path: &Path, text: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(path)?;
    writeln!(file, "{text}")?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oblivio_attestation::ed25519_dalek::SigningKey;

    #[test]
    fn lists_grow_with_increasing_sequence() {
        let first = next_list(None, "OBL-B", 50, 100).expect("first");
        assert_eq!(first.seq, 1);
        let second = next_list(Some(&first), " OBL-A ", 40, 200).expect("second");
        assert_eq!(second.seq, 2);
        assert_eq!(second.issued_at, 200);
        let ids: Vec<&str> = second
            .entries
            .iter()
            .map(|e| e.license_id.as_str())
            .collect();
        assert_eq!(ids, ["OBL-A", "OBL-B"]);

        // Signed and read back by the shipped decoder.
        let root = SigningKey::from_bytes(&[1; 32]);
        let text = second.sign(&root).expect("signed");
        assert_eq!(
            RevocationList::decode(&text, &root.verifying_key()).expect("decoded"),
            second
        );
    }

    #[test]
    fn a_license_is_revoked_once() {
        let first = next_list(None, "OBL-A", 50, 100).expect("first");
        assert!(next_list(Some(&first), "OBL-A", 10, 200).is_err());
        assert!(next_list(None, "  ", 10, 200).is_err());
    }

    #[test]
    fn register_lookup_and_no_overwrite() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let register = dir.path().join("issued-licenses.csv");
        std::fs::write(
            &register,
            "id,customer,offer,not_before,expires,workstation_key,issued_at\nOBL-A,x,Pro,1,,ab,2\n",
        )
        .expect("register");
        assert!(registered(&register, "OBL-A").expect("read"));
        assert!(!registered(&register, "OBL-Z").expect("read"));

        let out = dir.path().join("revocations.obr");
        write_new(&out, "OBR1:x").expect("written");
        assert!(write_new(&out, "OBR1:y").is_err(), "never overwritten");
    }
}
