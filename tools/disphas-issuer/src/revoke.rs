//! New revocation lists: the previous list plus one revoked license, with the
//! next sequence number. Lists only grow; a revocation cannot be undone.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use disphas_attestation::Revocation;
use disphas_attestation::revocation::RevocationList;

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

/// A new list replaces the previous one entirely: built from anything but
/// the latest list, it would silently drop revocations. So the latest list
/// is required (`--list`), unless the publisher states this is the very
/// first one (`--first`), which is refused once a list has been shipped.
/// `shipped` and `given` are sequence numbers.
pub fn check_base(shipped: Option<u64>, given: Option<u64>, first: bool) -> Result<(), String> {
    match (given, first) {
        (Some(_), true) => Err("give either --list or --first, not both".to_owned()),
        (None, false) => Err(
            "give the latest revocation list with --list, or --first for the very first list"
                .to_owned(),
        ),
        (None, true) => match shipped {
            Some(seq) => Err(format!(
                "revocation list #{seq} is already shipped: give the latest with --list"
            )),
            None => Ok(()),
        },
        (Some(given), false) => match shipped {
            Some(seq) if given < seq => Err(format!(
                "list #{given} is older than the shipped list #{seq}"
            )),
            _ => Ok(()),
        },
    }
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
    use disphas_attestation::ed25519_dalek::SigningKey;

    #[test]
    fn lists_grow_with_increasing_sequence() {
        let first = next_list(None, "DSL-B", 50, 100).expect("first");
        assert_eq!(first.seq, 1);
        let second = next_list(Some(&first), " DSL-A ", 40, 200).expect("second");
        assert_eq!(second.seq, 2);
        assert_eq!(second.issued_at, 200);
        let ids: Vec<&str> = second
            .entries
            .iter()
            .map(|e| e.license_id.as_str())
            .collect();
        assert_eq!(ids, ["DSL-A", "DSL-B"]);

        // Signed and read back by the shipped decoder.
        let root = SigningKey::from_bytes(&[1; 32]);
        let text = second.sign(&root).expect("signed");
        assert_eq!(
            RevocationList::decode(&text, &root.verifying_key()).expect("decoded"),
            second
        );
    }

    #[test]
    fn the_latest_list_is_required_unless_first() {
        assert!(check_base(None, None, false).is_err(), "nothing given");
        assert!(check_base(None, None, true).is_ok(), "very first list");
        assert!(
            check_base(Some(3), None, true).is_err(),
            "a list was shipped"
        );
        assert!(
            check_base(Some(3), Some(2), false).is_err(),
            "older than shipped"
        );
        assert!(check_base(Some(3), Some(3), false).is_ok());
        assert!(check_base(None, Some(1), false).is_ok());
        assert!(check_base(None, Some(1), true).is_err(), "both");
    }

    #[test]
    fn a_license_is_revoked_once() {
        let first = next_list(None, "DSL-A", 50, 100).expect("first");
        assert!(next_list(Some(&first), "DSL-A", 10, 200).is_err());
        assert!(next_list(None, "  ", 10, 200).is_err());
    }

    #[test]
    fn register_lookup_and_no_overwrite() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let register = dir.path().join("issued-licenses.csv");
        std::fs::write(
            &register,
            "id,customer,offer,not_before,expires,workstation_key,issued_at\nDSL-A,x,Pro,1,,ab,2\n",
        )
        .expect("register");
        assert!(registered(&register, "DSL-A").expect("read"));
        assert!(!registered(&register, "DSL-Z").expect("read"));

        let out = dir.path().join("revocations.dsr");
        write_new(&out, "DSR1:x").expect("written");
        assert!(write_new(&out, "DSR1:y").is_err(), "never overwritten");
    }
}
