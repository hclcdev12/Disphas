//! Activation and installation round trips on fictional keys.

use oblivio_attestation::ed25519_dalek::SigningKey;
use tempfile::TempDir;

use super::*;

const NOW: u64 = 1_791_158_400; // 2026-10-05T00:00:00Z
const DAY: u64 = 86_400;

/// Signs a license with the root key, as the issuer does. Issuing licenses
/// is not part of the product.
fn issue(license: &License, root: &SigningKey) -> String {
    license_text(&oblivio_attestation::cose::sign(license.to_cbor(), root).expect("signed"))
}

fn root() -> SigningKey {
    SigningKey::from_bytes(&[1; 32])
}

fn store() -> (TempDir, LicenseStore) {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = LicenseStore::new(dir.path());
    (dir, store)
}

/// What the issuer does with a request: a license for the requesting key.
fn license_for(request: &str, expires: Option<u64>) -> License {
    let request = read_activation_request(request).expect("valid request");
    License {
        id: "LIC-0001".to_owned(),
        customer: "Atelier XYZ".to_owned(),
        offer: "Pro".to_owned(),
        not_before: NOW - DAY,
        expires,
        workstation_key: request.workstation_key,
        issuer: None,
    }
}

fn activated(expires: Option<u64>) -> (TempDir, LicenseStore, String) {
    let (dir, store) = store();
    let request = store.create_activation_request(NOW).expect("request");
    let text = issue(&license_for(&request, expires), &root());
    (dir, store, text)
}

#[test]
fn full_activation_round_trip() {
    let (_dir, store, text) = activated(Some(NOW + 365 * DAY));
    assert_eq!(
        store
            .status(&root().verifying_key(), NOW, &[])
            .expect("status"),
        LicenseStatus::Missing
    );
    let installed = store
        .install(&text, &root().verifying_key(), NOW)
        .expect("installed");
    assert_eq!(installed.license.customer, "Atelier XYZ");
    match store
        .status(&root().verifying_key(), NOW, &[])
        .expect("status")
    {
        LicenseStatus::Valid(valid) => assert_eq!(valid, installed),
        other => panic!("expected Valid, got {other:?}"),
    }
}

#[test]
fn activation_request_reuses_the_existing_key() {
    let (_dir, store) = store();
    let first = read_activation_request(&store.create_activation_request(NOW).expect("request"))
        .expect("valid");
    let second =
        read_activation_request(&store.create_activation_request(NOW + 1).expect("request"))
            .expect("valid");
    assert_eq!(first.workstation_key, second.workstation_key);
}

#[test]
fn activation_request_must_be_signed_by_its_own_key() {
    let (_dir, store) = store();
    let request = store.create_activation_request(NOW).expect("request");
    let mut signed = base45::decode(&request[REQUEST_PREFIX.len()..]).expect("base45");
    let last = signed.len() - 1;
    signed[last] ^= 0x01; // flip a signature bit
    let forged = format!("{REQUEST_PREFIX}{}", base45::encode(&signed));
    assert!(matches!(
        read_activation_request(&forged),
        Err(LicenseError::Signature)
    ));
}

#[test]
fn key_and_license_files_are_private() {
    let (dir, store, text) = activated(None);
    store
        .install(&text, &root().verifying_key(), NOW)
        .expect("installed");
    for (path, expected) in [
        ("keys", 0o700),
        ("keys/workstation.key", 0o600),
        ("license", 0o700),
        ("license/oblivio.lic", 0o600),
    ] {
        let mode = fs::metadata(dir.path().join(path))
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, expected, "{path}");
    }
}

#[test]
fn license_signed_by_another_root_is_refused() {
    let (_dir, store) = store();
    let request = store.create_activation_request(NOW).expect("request");
    let forged = issue(
        &license_for(&request, None),
        &SigningKey::from_bytes(&[9; 32]),
    );
    assert!(matches!(
        store.install(&forged, &root().verifying_key(), NOW),
        Err(LicenseError::Signature)
    ));
}

#[test]
fn license_for_another_workstation_is_refused() {
    let (_dir, store) = store();
    store.create_activation_request(NOW).expect("request");
    let other = SigningKey::from_bytes(&[5; 32]);
    let license = License {
        workstation_key: other.verifying_key().to_bytes(),
        issuer: None,
        ..license_for(
            &store.create_activation_request(NOW).expect("request"),
            None,
        )
    };
    assert!(matches!(
        store.install(&issue(&license, &root()), &root().verifying_key(), NOW),
        Err(LicenseError::WrongWorkstation)
    ));
}

#[test]
fn license_before_activation_is_refused() {
    let (_dir, store) = store();
    let other = SigningKey::from_bytes(&[5; 32]);
    let license = License {
        id: "LIC-0002".to_owned(),
        customer: "x".to_owned(),
        offer: "Pro".to_owned(),
        not_before: NOW,
        expires: None,
        workstation_key: other.verifying_key().to_bytes(),
        issuer: None,
    };
    assert!(matches!(
        store.install(&issue(&license, &root()), &root().verifying_key(), NOW),
        Err(LicenseError::NoWorkstationKey)
    ));
}

#[test]
fn expired_license_is_refused_at_install() {
    let (_dir, store, text) = activated(Some(NOW - 1));
    assert!(matches!(
        store.install(&text, &root().verifying_key(), NOW),
        Err(LicenseError::Expired)
    ));
}

#[test]
fn status_follows_time() {
    let (_dir, store, text) = activated(Some(NOW + DAY));
    store
        .install(&text, &root().verifying_key(), NOW)
        .expect("installed");
    let root = root().verifying_key();
    assert!(matches!(
        store.status(&root, NOW - 2 * DAY, &[]).expect("status"),
        LicenseStatus::NotYetValid(_)
    ));
    assert!(matches!(
        store.status(&root, NOW + 2 * DAY, &[]).expect("status"),
        LicenseStatus::Expired(_)
    ));
}

#[test]
fn tampered_license_file_is_invalid() {
    let (dir, store, text) = activated(None);
    store
        .install(&text, &root().verifying_key(), NOW)
        .expect("installed");
    let path = dir.path().join("license/oblivio.lic");
    let mut content = fs::read_to_string(&path).expect("readable");
    let replaced = if content.ends_with('A') { 'B' } else { 'A' };
    content.pop();
    content.push(replaced);
    fs::write(&path, content).expect("tampered");
    assert!(matches!(
        store
            .status(&root().verifying_key(), NOW, &[])
            .expect("status"),
        LicenseStatus::Invalid(_)
    ));
}

#[test]
fn license_from_a_lost_key_is_invalid() {
    let (dir, store, text) = activated(None);
    store
        .install(&text, &root().verifying_key(), NOW)
        .expect("installed");
    fs::remove_file(dir.path().join("keys/workstation.key")).expect("key removed");
    assert!(matches!(
        store
            .status(&root().verifying_key(), NOW, &[])
            .expect("status"),
        LicenseStatus::Invalid(_)
    ));
}

#[test]
fn foreign_or_oversized_text_is_refused() {
    let root = root().verifying_key();
    assert!(matches!(
        decode_license("HC1:ABC", &root),
        Err(LicenseError::Format(_))
    ));
    let huge = format!("{LICENSE_PREFIX}{}", "0".repeat(MAX_TEXT_BYTES));
    assert!(matches!(
        decode_license(&huge, &root),
        Err(LicenseError::Format(_))
    ));
}

#[test]
fn development_root_issues_installable_licenses() {
    let (_dir, store) = store();
    let request = store.create_activation_request(NOW).expect("request");
    let text = crate::dev_root::issue(&request, NOW, 30).expect("issued");
    let installed = store
        .install(&text, &crate::dev_root::verifying_key(), NOW)
        .expect("installed");
    assert_eq!(installed.license.offer, "Dev");
    assert!(matches!(
        store.install(&text, &root().verifying_key(), NOW),
        Err(LicenseError::Signature)
    ));
}

#[test]
fn revoked_license_is_reported_from_its_revocation_time() {
    let (_dir, store, text) = activated(None);
    let installed = store
        .install(&text, &root().verifying_key(), NOW)
        .expect("installed");
    let revoked = |at| {
        vec![oblivio_attestation::Revocation {
            license_id: installed.license.id.clone(),
            revoked_at: at,
        }]
    };
    let status = |revocations: &[oblivio_attestation::Revocation]| {
        store
            .status(&root().verifying_key(), NOW, revocations)
            .expect("status")
    };
    assert!(
        matches!(status(&revoked(NOW - DAY)), LicenseStatus::Revoked(_, at) if at == NOW - DAY)
    );
    assert!(matches!(status(&revoked(NOW)), LicenseStatus::Revoked(..)));
    // Revoked in the future (scheduled end): still valid until then.
    assert!(matches!(
        status(&revoked(NOW + DAY)),
        LicenseStatus::Valid(_)
    ));
    // Another license revoked: no effect.
    let other = [oblivio_attestation::Revocation {
        license_id: "OBL-OTHER".to_owned(),
        revoked_at: 0,
    }];
    assert!(matches!(status(&other), LicenseStatus::Valid(_)));
}

// --- Licenses signed by a shop key (version 2).

fn shop() -> SigningKey {
    SigningKey::from_bytes(&[5; 32])
}

fn issuer() -> Issuer {
    Issuer {
        id: "ISS-2026-01".to_owned(),
        key: shop().verifying_key().to_bytes(),
        not_before: NOW - 30 * DAY,
        expires: NOW + 365 * DAY,
        offers: vec!["Atelier".to_owned(), "Pro".to_owned()],
        max_duration: 90 * DAY,
    }
}

/// What the license server does: a v2 license for the requesting key,
/// signed by the shop key, its issuer certificate signed by `root`.
fn shop_issue(request: &str, expires: u64, root: &SigningKey) -> String {
    let certificate =
        oblivio_attestation::cose::sign(issuer().to_cbor(), root).expect("signed issuer");
    let license = License {
        id: "OBL-20261005-0123456789abcdef".to_owned(),
        expires: Some(expires),
        issuer: Some(certificate),
        ..license_for(request, None)
    };
    issue(&license, &shop())
}

#[test]
fn shop_license_installs_and_renews() {
    let (_dir, store) = store();
    let request = store.create_activation_request(NOW).expect("request");
    let root_key = root().verifying_key();
    let first = store
        .install(
            &shop_issue(&request, NOW + 37 * DAY, &root()),
            &root_key,
            NOW,
        )
        .expect("installed");
    assert_eq!(first.issuer, Some(issuer()));
    assert!(matches!(
        store.status(&root_key, NOW, &[]).expect("status"),
        LicenseStatus::Valid(_)
    ));
    // Renewal: same seat, later end, installed over the previous license
    // without a new activation request.
    let renewed = store
        .install(
            &shop_issue(&request, NOW + 67 * DAY, &root()),
            &root_key,
            NOW,
        )
        .expect("renewed");
    assert_eq!(renewed.license.id, first.license.id);
    match store
        .status(&root_key, NOW + 50 * DAY, &[])
        .expect("status")
    {
        LicenseStatus::Valid(valid) => assert_eq!(valid.license.expires, Some(NOW + 67 * DAY)),
        other => panic!("expected Valid, got {other:?}"),
    }
}

#[test]
fn shop_license_needs_an_issuer_certified_by_the_root() {
    let (_dir, store) = store();
    let request = store.create_activation_request(NOW).expect("request");
    // Issuer certificate signed by the shop key itself.
    assert!(matches!(
        store.install(
            &shop_issue(&request, NOW + 37 * DAY, &shop()),
            &root().verifying_key(),
            NOW
        ),
        Err(LicenseError::Signature)
    ));
    // Longer than the issuer allows (90 days).
    assert!(matches!(
        store.install(
            &shop_issue(&request, NOW + 120 * DAY, &root()),
            &root().verifying_key(),
            NOW
        ),
        Err(LicenseError::Content)
    ));
}

#[test]
fn revoked_issuer_revokes_its_licenses() {
    let (_dir, store) = store();
    let request = store.create_activation_request(NOW).expect("request");
    let root_key = root().verifying_key();
    store
        .install(
            &shop_issue(&request, NOW + 37 * DAY, &root()),
            &root_key,
            NOW,
        )
        .expect("installed");
    let issuer_revoked = [oblivio_attestation::Revocation {
        license_id: "ISS-2026-01".to_owned(),
        revoked_at: NOW - DAY,
    }];
    assert!(matches!(
        store.status(&root_key, NOW, &issuer_revoked).expect("status"),
        LicenseStatus::Revoked(_, at) if at == NOW - DAY
    ));
    // A root-signed license ignores issuer revocations.
    let (_dir, store, text) = activated(None);
    store.install(&text, &root_key, NOW).expect("installed");
    assert!(matches!(
        store
            .status(&root_key, NOW, &issuer_revoked)
            .expect("status"),
        LicenseStatus::Valid(_)
    ));
}
