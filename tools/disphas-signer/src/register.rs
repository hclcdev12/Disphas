//! The register of signed licenses (`docs/licences_en_ligne.md` §6.5):
//! append-only CSV, one line per signature, written and synced before the
//! license is handed out. A renewal adds a line with the same identifier.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use disphas_attestation::License;

const HEADER: &str = "id,customer,offer,not_before,expires,workstation_key,issuer,signed_at";

/// What a renewal keeps from the first signature of a license.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    /// Customer name.
    pub customer: String,
    /// Commercial offer.
    pub offer: String,
    /// Workstation public key.
    pub workstation_key: [u8; 32],
}

/// The register file and what was read from it.
#[derive(Debug)]
pub struct Register {
    path: PathBuf,
    seats: HashMap<String, Seat>,
    signed_at: Vec<u64>,
}

impl Register {
    /// Opens the register at `path`, reading every line; creates it if absent.
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut register = Self {
            path: path.to_owned(),
            seats: HashMap::new(),
            signed_at: Vec::new(),
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)?;
                writeln!(file, "{HEADER}")?;
                file.sync_all()?;
                sync_parent(path)?;
                return Ok(register);
            }
            Err(error) => return Err(error),
        };
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return Err(invalid("unexpected register header"));
        }
        for (number, line) in lines.enumerate() {
            let fields = split(line).ok_or_else(|| invalid(&format!("line {}", number + 2)))?;
            let [id, customer, offer, _, _, key, _, signed_at] = fields.as_slice() else {
                return Err(invalid(&format!("line {}", number + 2)));
            };
            let workstation_key =
                parse_key(key).ok_or_else(|| invalid(&format!("line {}", number + 2)))?;
            let signed_at = signed_at
                .parse()
                .map_err(|_| invalid(&format!("line {}", number + 2)))?;
            register.seats.insert(
                id.clone(),
                Seat {
                    customer: customer.clone(),
                    offer: offer.clone(),
                    workstation_key,
                },
            );
            register.signed_at.push(signed_at);
        }
        Ok(register)
    }

    /// The seat of license `id`, if it was ever signed.
    pub fn seat(&self, id: &str) -> Option<&Seat> {
        self.seats.get(id)
    }

    /// Whether `id` was ever signed.
    pub fn contains(&self, id: &str) -> bool {
        self.seats.contains_key(id)
    }

    /// Signatures made at or after `since` (UTC epoch seconds).
    pub fn signed_since(&self, since: u64) -> usize {
        self.signed_at.iter().filter(|at| **at >= since).count()
    }

    /// Appends a signed license and syncs it to disk.
    pub fn append(&mut self, license: &License, issuer: &str, signed_at: u64) -> io::Result<()> {
        let key: String = license
            .workstation_key
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let line = format!(
            "{},{},{},{},{},{key},{},{signed_at}",
            field(&license.id),
            field(&license.customer),
            field(&license.offer),
            license.not_before,
            license.expires.map(|at| at.to_string()).unwrap_or_default(),
            field(issuer),
        );
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        writeln!(file, "{line}")?;
        file.sync_all()?;
        self.seats.insert(
            license.id.clone(),
            Seat {
                customer: license.customer.clone(),
                offer: license.offer.clone(),
                workstation_key: license.workstation_key,
            },
        );
        self.signed_at.push(signed_at);
        Ok(())
    }
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("register: {what}"))
}

/// One CSV field, quoted if needed.
fn field(value: &str) -> String {
    if value.contains([',', '"']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Splits one CSV line (quoted fields, no line breaks inside a field).
fn split(line: &str) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (false, ',') => fields.push(std::mem::take(&mut current)),
            (false, '"') if current.is_empty() => quoted = true,
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
            }
            (true, '"') => {
                quoted = false;
                if !matches!(chars.peek(), None | Some(',')) {
                    return None;
                }
            }
            (_, c) => current.push(c),
        }
    }
    if quoted {
        return None;
    }
    fields.push(current);
    Some(fields)
}

fn parse_key(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(2 * index..2 * index + 2)?, 16).ok()?;
    }
    Some(key)
}

fn sync_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) => File::open(parent)?.sync_all(),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn license(id: &str, customer: &str) -> License {
        License {
            id: id.to_owned(),
            customer: customer.to_owned(),
            offer: "Pro".to_owned(),
            not_before: 100,
            expires: Some(200),
            workstation_key: [7; 32],
            issuer: None,
        }
    }

    #[test]
    fn signatures_survive_a_restart() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("register.csv");
        let mut register = Register::open(&path).expect("created");
        register
            .append(&license("DSL-A", "Atelier, \"XYZ\""), "ISS-1", 150)
            .expect("appended");
        register
            .append(&license("DSL-A", "Atelier, \"XYZ\""), "ISS-1", 180)
            .expect("renewed");
        let reopened = Register::open(&path).expect("reopened");
        let seat = reopened.seat("DSL-A").expect("seat");
        assert_eq!(seat.customer, "Atelier, \"XYZ\"");
        assert_eq!(seat.workstation_key, [7; 32]);
        assert_eq!(reopened.signed_since(160), 1);
        assert!(!reopened.contains("DSL-B"));
    }

    #[test]
    fn a_damaged_register_is_refused() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("register.csv");
        std::fs::write(&path, format!("{HEADER}\nDSL-A,\"unterminated,Pro\n")).expect("written");
        assert!(Register::open(&path).is_err());
        std::fs::write(&path, "id,other\n").expect("written");
        assert!(Register::open(&path).is_err());
    }
}
