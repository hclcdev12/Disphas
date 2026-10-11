//! The signing service on fictional keys: a test root, shop keys generated
//! in a temporary state directory, workstations in temporary stores.

use std::os::unix::fs::PermissionsExt;

use oblivio_attestation::ed25519_dalek::SigningKey;
use oblivio_attestation::{Issuer, cose};
use oblivio_license::{LicenseStatus, LicenseStore, issuer_text};
use tempfile::TempDir;

use super::*;
use crate::store::Store;

const NOW: u64 = 1_791_158_400; // 2026-10-05T00:00:00Z
const DAY: u64 = 86_400;

fn root() -> SigningKey {
    SigningKey::from_bytes(&[1; 32])
}

fn private_tempdir() -> TempDir {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).expect("chmod");
    dir
}

/// Generates a shop key in `store` and installs a certificate for it.
fn certify(store: &Store, id: &str, not_before: u64, days: u64) {
    let key = store.keygen().expect("shop key");
    let issuer = Issuer {
        id: id.to_owned(),
        key: key.to_bytes(),
        not_before,
        expires: not_before + days * DAY,
        offers: vec!["Atelier".to_owned(), "Pro".to_owned()],
        max_duration: 90 * DAY,
    };
    let signed = cose::sign(issuer.to_cbor(), &root()).expect("signed issuer");
    store
        .install(&issuer_text(&signed), &[root().verifying_key()], NOW)
        .expect("installed");
}

/// A service with one certificate valid around `NOW`.
fn service() -> (TempDir, Store, Signer) {
    let dir = private_tempdir();
    let store = Store::new(dir.path().join("state"));
    certify(&store, "ISS-2026-01", NOW - 30 * DAY, 365);
    let signer = open(&store);
    (dir, store, signer)
}

fn open(store: &Store) -> Signer {
    store.prepare().expect("state directory");
    let roots = vec![root().verifying_key()];
    let certified = store.certified(&roots).expect("certificates");
    let register = Register::open(&store.register_path()).expect("register");
    Signer::new(certified, roots, register)
}

/// A workstation and its activation request.
fn workstation() -> (TempDir, LicenseStore, String) {
    let dir = private_tempdir();
    let store = LicenseStore::new(dir.path());
    let request = store.create_activation_request(NOW).expect("request");
    (dir, store, request)
}

fn activate(signer: &mut Signer, request: &str, offer: &str, expires: u64) -> Reply {
    signer.handle(
        Request::Activate {
            request: request.to_owned(),
            customer: "Atelier XYZ".to_owned(),
            offer: offer.to_owned(),
            expires,
        },
        NOW,
    )
}

fn error_of(reply: &Reply) -> Option<ErrorCode> {
    match reply {
        Reply::Error { error, .. } => Some(*error),
        _ => None,
    }
}

#[test]
fn activated_license_installs_on_the_workstation_and_is_recorded() {
    let (_dir, store, mut signer) = service();
    let (_ws, workstation, request) = workstation();
    let Reply::Activated {
        id,
        workstation_key,
        license,
    } = activate(&mut signer, &request, "Pro", NOW + 37 * DAY)
    else {
        panic!("not activated");
    };
    assert!(id.starts_with("OBL-20261005-") && id.len() == 29, "{id}");
    let installed = workstation
        .install(&license, &root().verifying_key(), NOW)
        .expect("installed");
    assert_eq!(installed.license.id, id);
    assert_eq!(
        installed.issuer.map(|issuer| issuer.id).as_deref(),
        Some("ISS-2026-01")
    );
    let expected: String = installed
        .license
        .workstation_key
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(workstation_key, expected);
    // Recorded, and still known after a restart.
    assert!(open(&store).register.contains(&id));
}

#[test]
fn renewal_only_moves_the_end_date() {
    let (_dir, _store, mut signer) = service();
    let (_ws, workstation, request) = workstation();
    let Reply::Activated { id, .. } = activate(&mut signer, &request, "Atelier", NOW + 37 * DAY)
    else {
        panic!("not activated");
    };
    let Reply::Renewed { license } = signer.handle(
        Request::Renew {
            id: id.clone(),
            expires: NOW + 67 * DAY,
        },
        NOW + 30 * DAY,
    ) else {
        panic!("not renewed");
    };
    let installed = workstation
        .install(&license, &root().verifying_key(), NOW + 30 * DAY)
        .expect("installed");
    assert_eq!(installed.license.id, id);
    assert_eq!(installed.license.offer, "Atelier");
    assert_eq!(installed.license.customer, "Atelier XYZ");
    assert_eq!(installed.license.expires, Some(NOW + 67 * DAY));
    assert!(matches!(
        workstation
            .status(&root().verifying_key(), NOW + 50 * DAY, &[])
            .expect("status"),
        LicenseStatus::Valid(_)
    ));
    assert_eq!(
        error_of(&signer.handle(
            Request::Renew {
                id: "OBL-UNKNOWN".to_owned(),
                expires: NOW + 30 * DAY,
            },
            NOW,
        )),
        Some(ErrorCode::UnknownLicense)
    );
}

#[test]
fn requests_outside_the_bounds_are_refused() {
    let (_dir, _store, mut signer) = service();
    let (_ws, _workstation, request) = workstation();
    let cases = [
        (
            activate(&mut signer, &request, "Grands comptes", NOW + 30 * DAY),
            ErrorCode::OutOfBounds,
        ),
        (
            activate(&mut signer, &request, "Pro", NOW + 91 * DAY),
            ErrorCode::OutOfBounds,
        ),
        (
            activate(&mut signer, &request, "Pro", NOW),
            ErrorCode::BadRequest,
        ),
        (
            activate(&mut signer, "OBA1:forged", "Pro", NOW + DAY),
            ErrorCode::BadRequest,
        ),
    ];
    for (reply, expected) in cases {
        assert_eq!(error_of(&reply), Some(expected), "{reply:?}");
    }
    for customer in [String::new(), "A\u{7}".to_owned(), "x".repeat(129)] {
        let reply = signer.handle(
            Request::Activate {
                request: request.clone(),
                customer,
                offer: "Pro".to_owned(),
                expires: NOW + DAY,
            },
            NOW,
        );
        assert_eq!(error_of(&reply), Some(ErrorCode::OutOfBounds));
    }
    // Nothing refused was recorded.
    assert_eq!(signer.register.signed_since(0), 0);
}

#[test]
fn without_a_valid_certificate_nothing_is_signed() {
    let dir = private_tempdir();
    let store = Store::new(dir.path().join("state"));
    let (_ws, _workstation, request) = workstation();
    assert_eq!(
        error_of(&activate(&mut open(&store), &request, "Pro", NOW + DAY)),
        Some(ErrorCode::NoIssuer)
    );
    // Expired certificates cannot even be installed.
    let key = store.keygen().expect("shop key");
    let expired = Issuer {
        id: "ISS-OLD".to_owned(),
        key: key.to_bytes(),
        not_before: NOW - 400 * DAY,
        expires: NOW - DAY,
        offers: vec!["Pro".to_owned()],
        max_duration: 90 * DAY,
    };
    let signed = cose::sign(expired.to_cbor(), &root()).expect("signed");
    assert!(
        store
            .install(&issuer_text(&signed), &[root().verifying_key()], NOW)
            .is_err()
    );
}

#[test]
fn certificates_are_checked_at_installation() {
    let dir = private_tempdir();
    let store = Store::new(dir.path().join("state"));
    let held = store.keygen().expect("shop key");
    let stranger = SigningKey::from_bytes(&[9; 32]);
    let certificate = |key: [u8; 32], signer: &SigningKey| {
        let issuer = Issuer {
            id: "ISS-2026-01".to_owned(),
            key,
            not_before: NOW,
            expires: NOW + 365 * DAY,
            offers: vec!["Pro".to_owned()],
            max_duration: 90 * DAY,
        };
        issuer_text(&cose::sign(issuer.to_cbor(), signer).expect("signed"))
    };
    let roots = [root().verifying_key()];
    // Not signed by the root.
    assert!(
        store
            .install(&certificate(held.to_bytes(), &stranger), &roots, NOW)
            .is_err()
    );
    // For a key not held here.
    assert!(
        store
            .install(
                &certificate(stranger.verifying_key().to_bytes(), &root()),
                &roots,
                NOW
            )
            .is_err()
    );
    // Valid, then not twice.
    store
        .install(&certificate(held.to_bytes(), &root()), &roots, NOW)
        .expect("installed");
    assert!(
        store
            .install(&certificate(held.to_bytes(), &root()), &roots, NOW)
            .is_err()
    );
}

#[test]
fn the_latest_valid_certificate_signs() {
    let (_dir, store, _) = service();
    // The next certificate, installed 90 days ahead of the rotation.
    certify(&store, "ISS-2027-01", NOW - DAY, 400);
    let mut signer = open(&store);
    let (_ws, workstation, request) = workstation();
    let Reply::Activated { license, .. } = activate(&mut signer, &request, "Pro", NOW + 37 * DAY)
    else {
        panic!("not activated");
    };
    let installed = workstation
        .install(&license, &root().verifying_key(), NOW)
        .expect("installed");
    assert_eq!(
        installed.issuer.map(|issuer| issuer.id).as_deref(),
        Some("ISS-2027-01")
    );
    let Reply::Status { issuers, .. } = signer.handle(Request::Status, NOW) else {
        panic!("no status");
    };
    let current: Vec<&str> = issuers
        .iter()
        .filter(|issuer| issuer.current)
        .map(|issuer| issuer.id.as_str())
        .collect();
    assert_eq!(current, ["ISS-2027-01"]);
}

#[test]
fn signature_rate_is_bounded() {
    let (_dir, store, mut signer) = service();
    let (_ws, _workstation, request) = workstation();
    for _ in 0..MAX_PER_MINUTE {
        assert!(matches!(
            activate(&mut signer, &request, "Pro", NOW + DAY),
            Reply::Activated { .. }
        ));
    }
    assert_eq!(
        error_of(&activate(&mut signer, &request, "Pro", NOW + DAY)),
        Some(ErrorCode::RateLimited)
    );
    // A minute later, signing resumes.
    let reply = signer.handle(
        Request::Activate {
            request: request.clone(),
            customer: "Atelier XYZ".to_owned(),
            offer: "Pro".to_owned(),
            expires: NOW + DAY,
        },
        NOW + 60,
    );
    assert!(matches!(reply, Reply::Activated { .. }), "{reply:?}");

    // The daily bound counts the register, so it survives a restart.
    let path = store.register_path();
    let mut text = std::fs::read_to_string(&path).expect("register");
    let line = text.lines().nth(1).expect("a line").to_owned();
    for _ in 0..MAX_PER_DAY {
        text.push_str(&line);
        text.push('\n');
    }
    std::fs::write(&path, text).expect("written");
    let mut restarted = open(&store);
    let reply = restarted.handle(
        Request::Activate {
            request,
            customer: "Atelier XYZ".to_owned(),
            offer: "Pro".to_owned(),
            expires: NOW + DAY,
        },
        NOW + 3600,
    );
    assert_eq!(error_of(&reply), Some(ErrorCode::RateLimited));
}

#[test]
fn protocol_lines_are_strict() {
    let parse = |line: &str| serde_json::from_str::<Request>(line);
    assert_eq!(
        parse(r#"{"cmd":"Status"}"#).expect("status"),
        Request::Status
    );
    assert_eq!(
        parse(r#"{"cmd":"Renew","id":"OBL-A","expires":5}"#).expect("renew"),
        Request::Renew {
            id: "OBL-A".to_owned(),
            expires: 5
        }
    );
    assert!(parse(r#"{"cmd":"Renew","id":"OBL-A","expires":5,"offer":"Pro"}"#).is_err());
    assert!(parse(r#"{"cmd":"Sign"}"#).is_err());
    let reply = serde_json::to_string(&Reply::Error {
        error: ErrorCode::RateLimited,
        detail: "x".to_owned(),
    })
    .expect("serialized");
    assert_eq!(reply, r#"{"error":"RateLimited","detail":"x"}"#);
}

#[test]
fn the_socket_answers_line_by_line() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::Mutex;

    let (_dir, _store, signer) = service();
    let signer = Mutex::new(signer);
    let (mut client, server) = UnixStream::pair().expect("socket pair");
    std::thread::scope(|scope| {
        scope.spawn(|| crate::serve::connection(server, &signer, || NOW));
        client
            .write_all(b"{\"cmd\":\"Status\"}\nnot json\n")
            .expect("written");
        let mut reader = BufReader::new(client.try_clone().expect("clone"));
        let mut first = String::new();
        reader.read_line(&mut first).expect("first reply");
        assert!(first.contains("\"issuers\""), "{first}");
        let mut second = String::new();
        reader.read_line(&mut second).expect("second reply");
        assert!(second.contains("\"BadRequest\""), "{second}");
        // An overlong line is answered once, then the connection closes.
        client.write_all(&vec![b'x'; 17 * 1024]).expect("written");
        let mut third = String::new();
        reader.read_line(&mut third).expect("third reply");
        assert!(third.contains("request too long"), "{third}");
        drop(client);
    });
}
