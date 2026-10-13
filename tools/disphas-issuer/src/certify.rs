//! Issuer certificates: the root key delegates license signing to a shop
//! key, within bounds (`docs/licences_en_ligne.md` §3). The shop key is
//! generated on the license server and never leaves it; only its public half
//! comes here. One certificate a year, with overlap for the rotation.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use disphas_attestation::ed25519_dalek::{SigningKey, VerifyingKey};
use disphas_attestation::model::ISSUER_ID_PREFIX;
use disphas_attestation::{Issuer, cose};
use disphas_license::{decode_issuer, issuer_text};

/// What the publisher decides for an issuer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// Issuer identifier, `ISS-…`.
    pub id: String,
    /// Shop public key.
    pub key: VerifyingKey,
    /// Start of validity, UTC epoch seconds.
    pub not_before: u64,
    /// End of validity, UTC epoch seconds.
    pub expires: u64,
    /// Offers the shop may sign.
    pub offers: Vec<String>,
    /// Longest license the shop may sign, in seconds.
    pub max_duration: u64,
}

/// Parses a shop public key given as 64 hexadecimal digits.
pub fn parse_key(hex: &str) -> Result<VerifyingKey, String> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("the shop key must be 64 hexadecimal digits".to_owned());
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * index..2 * index + 2], 16)
            .map_err(|_| "the shop key must be 64 hexadecimal digits".to_owned())?;
    }
    VerifyingKey::from_bytes(&bytes).map_err(|_| "the shop key is not an Ed25519 key".to_owned())
}

/// Signs an issuer certificate (`DSI1:…`) with the root key, then reads it
/// back as the license server will.
pub fn certify(root: &SigningKey, grant: &Grant) -> Result<(Issuer, String), String> {
    if !grant.id.starts_with(ISSUER_ID_PREFIX) {
        return Err(format!(
            "an issuer identifier starts with {ISSUER_ID_PREFIX}"
        ));
    }
    if grant.key == root.verifying_key() {
        return Err("the shop key must not be the root key".to_owned());
    }
    let mut offers: Vec<String> = grant
        .offers
        .iter()
        .map(|offer| offer.trim().to_owned())
        .filter(|offer| !offer.is_empty())
        .collect();
    offers.sort();
    offers.dedup();
    let issuer = Issuer {
        id: grant.id.clone(),
        key: grant.key.to_bytes(),
        not_before: grant.not_before,
        expires: grant.expires,
        offers,
        max_duration: grant.max_duration,
    };
    // The decoder holds the rules (prefix, dates, offers, duration): a
    // certificate it refuses is never written.
    Issuer::from_cbor(&issuer.to_cbor()).map_err(|error| format!("invalid issuer: {error}"))?;
    let signed = cose::sign(issuer.to_cbor(), root).map_err(|error| error.to_string())?;
    let text = issuer_text(&signed);
    let (read_back, _) =
        decode_issuer(&text, &root.verifying_key()).map_err(|error| error.to_string())?;
    if read_back != issuer {
        return Err("the issuer certificate does not read back".to_owned());
    }
    Ok((issuer, text))
}

/// Whether `id` is in the issuers register (first column).
pub fn registered(register: &Path, id: &str) -> std::io::Result<bool> {
    crate::revoke::registered(register, id)
}

/// Appends a certified issuer to the CSV register, creating it with a header.
pub fn record(register: &Path, issuer: &Issuer, issued_at: u64) -> std::io::Result<()> {
    let new = !register.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(register)?;
    if new {
        writeln!(
            file,
            "id,shop_key,not_before,expires,offers,max_duration,issued_at"
        )?;
    }
    let key: String = issuer
        .key
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    writeln!(
        file,
        "{},{key},{},{},{},{},{issued_at}",
        issuer.id,
        issuer.not_before,
        issuer.expires,
        issuer.offers.join(" "),
        issuer.max_duration,
    )?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use disphas_attestation::License;
    use disphas_license::LicenseStore;
    use std::os::unix::fs::PermissionsExt;

    const NOW: u64 = 1_791_158_400;
    const DAY: u64 = 86_400;

    fn root() -> SigningKey {
        SigningKey::from_bytes(&[42; 32])
    }

    fn shop() -> SigningKey {
        SigningKey::from_bytes(&[5; 32])
    }

    fn grant() -> Grant {
        Grant {
            id: "ISS-2026-01".to_owned(),
            key: shop().verifying_key(),
            not_before: NOW,
            expires: NOW + 365 * DAY,
            offers: vec!["Pro".to_owned(), " Atelier".to_owned(), "Pro".to_owned()],
            max_duration: 90 * DAY,
        }
    }

    #[test]
    fn certified_shop_key_signs_installable_licenses() {
        let (issuer, text) = certify(&root(), &grant()).expect("certified");
        assert!(text.starts_with("DSI1:"));
        assert_eq!(issuer.offers, ["Atelier", "Pro"]);

        // What the license server will do with the certificate.
        let (_, certificate) = decode_issuer(&text, &root().verifying_key()).expect("decoded");
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let store = LicenseStore::new(dir.path());
        let request = disphas_license::read_activation_request(
            &store.create_activation_request(NOW).expect("request"),
        )
        .expect("valid request");
        let license = License {
            id: "DSL-20261005-0123456789abcdef".to_owned(),
            customer: "Atelier XYZ".to_owned(),
            offer: "Pro".to_owned(),
            not_before: NOW,
            expires: Some(NOW + 37 * DAY),
            workstation_key: request.workstation_key,
            issuer: Some(certificate),
        };
        let signed = cose::sign(license.to_cbor(), &shop()).expect("signed");
        store
            .install(
                &disphas_license::license_text(&signed),
                &root().verifying_key(),
                NOW + 60,
            )
            .expect("installed");
    }

    #[test]
    fn invalid_grants_are_refused() {
        let root = root();
        for (grant, why) in [
            (
                Grant {
                    id: "SHOP-1".to_owned(),
                    ..grant()
                },
                "prefix",
            ),
            (
                Grant {
                    key: root.verifying_key(),
                    ..grant()
                },
                "root key as shop key",
            ),
            (
                Grant {
                    expires: NOW,
                    ..grant()
                },
                "empty validity",
            ),
            (
                Grant {
                    offers: vec![" ".to_owned()],
                    ..grant()
                },
                "no offer",
            ),
            (
                Grant {
                    max_duration: 0,
                    ..grant()
                },
                "zero duration",
            ),
        ] {
            assert!(certify(&root, &grant).is_err(), "{why}");
        }
    }

    #[test]
    fn shop_keys_are_read_from_hexadecimal() {
        let hex: String = shop()
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(parse_key(&hex), Ok(shop().verifying_key()));
        assert!(parse_key(&hex[..62]).is_err());
        assert!(parse_key(&"zz".repeat(32)).is_err());
    }

    #[test]
    fn register_keeps_one_line_per_issuer() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let register = dir.path().join("issuers.csv");
        let (issuer, _) = certify(&root(), &grant()).expect("certified");
        record(&register, &issuer, NOW).expect("recorded");
        assert!(registered(&register, "ISS-2026-01").expect("read"));
        assert!(!registered(&register, "ISS-2027-01").expect("read"));
        let text = std::fs::read_to_string(&register).expect("read");
        assert!(
            text.lines()
                .nth(1)
                .expect("line")
                .ends_with(",Atelier Pro,7776000,1791158400")
        );
    }
}
