//! License issuance from an activation request, and the register of issued
//! licenses (input for the future revocation list).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use oblivio_attestation::ed25519_dalek::SigningKey;
use oblivio_attestation::{License, cose};
use oblivio_license::{license_text, read_activation_request};

/// What the publisher decides for a license.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terms {
    /// License identifier.
    pub id: String,
    /// Customer name.
    pub customer: String,
    /// Commercial offer.
    pub offer: String,
    /// Start of validity, UTC epoch seconds.
    pub not_before: u64,
    /// End of validity, UTC epoch seconds; `None` for a perpetual license.
    pub expires: Option<u64>,
}

/// Signs a license (`OBL1:…`) for the workstation of `request`.
/// The request must be signed by the key it carries.
pub fn issue(root: &SigningKey, request: &str, terms: &Terms) -> Result<(License, String), String> {
    let request = read_activation_request(request).map_err(|error| error.to_string())?;
    if terms
        .expires
        .is_some_and(|expires| expires <= terms.not_before)
    {
        return Err("the license would expire before it starts".to_owned());
    }
    let license = License {
        id: terms.id.clone(),
        customer: terms.customer.clone(),
        offer: terms.offer.clone(),
        not_before: terms.not_before,
        expires: terms.expires,
        workstation_key: request.workstation_key,
    };
    let signed = cose::sign(license.to_cbor(), root).map_err(|error| error.to_string())?;
    Ok((license, license_text(&signed)))
}

/// One CSV field, quoted if needed.
fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Appends an issued license to the CSV register, creating it with a header.
pub fn record(register: &Path, license: &License, issued_at: u64) -> std::io::Result<()> {
    let new = !register.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(register)?;
    if new {
        writeln!(
            file,
            "id,customer,offer,not_before,expires,workstation_key,issued_at"
        )?;
    }
    let key: String = license
        .workstation_key
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    writeln!(
        file,
        "{},{},{},{},{},{key},{issued_at}",
        field(&license.id),
        field(&license.customer),
        field(&license.offer),
        license.not_before,
        license
            .expires
            .map(|expires| expires.to_string())
            .unwrap_or_default(),
    )?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oblivio_license::LicenseStore;
    use std::os::unix::fs::PermissionsExt;

    const NOW: u64 = 1_791_158_400;

    fn terms() -> Terms {
        Terms {
            id: "OBL-0001".to_owned(),
            customer: "Atelier, \"XYZ\"".to_owned(),
            offer: "Pro".to_owned(),
            not_before: NOW,
            expires: Some(NOW + 86_400 * 365),
        }
    }

    #[test]
    fn issued_license_installs_on_the_requesting_workstation() {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let store = LicenseStore::new(dir.path());
        let request = store.create_activation_request(NOW).expect("request");
        let root = SigningKey::from_bytes(&[42; 32]);
        let (license, text) = issue(&root, &request, &terms()).expect("issued");
        let installed = store
            .install(&text, &root.verifying_key(), NOW + 60)
            .expect("installed");
        assert_eq!(installed.license, license);
    }

    #[test]
    fn inconsistent_dates_are_refused() {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let request = LicenseStore::new(dir.path())
            .create_activation_request(NOW)
            .expect("request");
        let root = SigningKey::from_bytes(&[42; 32]);
        let terms = Terms {
            expires: Some(NOW),
            ..terms()
        };
        assert!(issue(&root, &request, &terms).is_err());
    }

    #[test]
    fn register_quotes_fields_and_keeps_appending() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let register = dir.path().join("issued.csv");
        let license = License {
            id: "OBL-0001".to_owned(),
            customer: "Atelier, \"XYZ\"".to_owned(),
            offer: "Pro".to_owned(),
            not_before: NOW,
            expires: None,
            workstation_key: [1; 32],
        };
        record(&register, &license, NOW).expect("recorded");
        record(&register, &license, NOW + 1).expect("recorded");
        let text = std::fs::read_to_string(&register).expect("read");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[1].starts_with("OBL-0001,\"Atelier, \"\"XYZ\"\"\",Pro,1791158400,,0101"));
    }
}
