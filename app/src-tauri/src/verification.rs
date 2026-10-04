//! Verification of a certificate given by the user: a PDF (its attached
//! attestation and journal), a binary COSE attestation or the QR text
//! (`OBV1:…`). Same checks as `oblivio-verify`: signature, license chain to a
//! trusted root, license validity at the attestation date, journal digest.

use oblivio_attestation::display::uuid;
use oblivio_attestation::pdf::{ExtractError, attachments};

use crate::trust::Trust;
use oblivio_attestation::{Verified, VerifyError, from_qr_text, verify};
use serde::Serialize;

/// Largest input accepted (a certificate PDF is well under 1 MiB).
pub const MAX_INPUT_BYTES: usize = 32 << 20;

/// What the user gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Source {
    /// A certificate PDF, through its attachments.
    Pdf,
    /// A binary COSE attestation.
    Cose,
    /// The QR text.
    Qr,
}

/// Root the certificate chains to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Root {
    /// The official Oblivio root key.
    Official,
    /// Another trusted root: the public development root (debug builds).
    Development,
}

/// Whether the journal matches the attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Journal {
    /// Its SHA-256 is the one signed.
    Matches,
    /// It is not the journal the attestation covers.
    Mismatch,
    /// No journal given (COSE or QR alone).
    Absent,
}

/// Validation as signed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Validation {
    /// `canaris` or `motif`.
    pub method: String,
    /// Number of sample blocks.
    pub samples: u32,
    /// Whether it passed.
    pub passed: bool,
}

/// Everything the attestation says, once verified. Codes stay codes: the
/// interface translates them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Details {
    /// Report identifier.
    pub id: String,
    /// End of the operation, UTC epoch seconds.
    pub issued_at: u64,
    /// `CONFORME`, `NON_CONFORME` or `REFUS`.
    pub status: String,
    /// `Clear`, `Purge` or `None`.
    pub category: String,
    /// Method code, if one ran.
    pub method: Option<String>,
    /// Firmware verification, if it ran.
    pub verified: Option<bool>,
    /// Validation, if it ran.
    pub validation: Option<Validation>,
    /// `Reuse` or `DestroyRecommended`, absent on a refusal.
    pub disposition: Option<String>,
    /// Disposition codes.
    pub disposition_codes: Vec<String>,
    /// Reserve codes.
    pub reserves: Vec<String>,
    /// Refusal codes.
    pub refusals: Vec<String>,
    /// Forcing justification, as typed.
    pub justification: Option<String>,
    /// Drive model.
    pub model: String,
    /// Drive serial number.
    pub serial: String,
    /// Drive firmware.
    pub firmware: String,
    /// Capacity in bytes.
    pub capacity_bytes: u64,
    /// `NVMe`, `SATA-SSD`, `SATA-HDD` or `Unknown`.
    pub drive_type: String,
    /// WWN, EUI-64 or NGUID, whichever is present first.
    pub hardware_id: Option<String>,
    /// Declarative operator.
    pub operator: Option<String>,
    /// Declarative organization.
    pub organization: Option<String>,
    /// Oblivio version.
    pub tool_version: String,
    /// Rule set version.
    pub rules_version: String,
    /// SHA-256 of the helper binary, hex.
    pub binary_sha256: String,
    /// License identifier.
    pub license_id: String,
    /// Licensed customer.
    pub customer: String,
}

/// Outcome of a verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// What was given, if recognized.
    pub source: Option<Source>,
    /// Whether the certificate is authentic.
    pub authentic: bool,
    /// Why not, as a code the interface translates.
    pub reason: Option<String>,
    /// Root it chains to, when authentic.
    pub root: Option<Root>,
    /// Journal check.
    pub journal: Journal,
    /// Content, only when authentic.
    pub details: Option<Details>,
}

fn reason(error: &VerifyError) -> &'static str {
    match error {
        VerifyError::NotOblivio => "NotOblivio",
        VerifyError::Corrupted => "Corrupted",
        VerifyError::AttestationEnvelope(_) | VerifyError::AttestationContent(_) => {
            "AttestationInvalid"
        }
        VerifyError::License(_) | VerifyError::LicenseContent(_) | VerifyError::WorkstationKey => {
            "LicenseInvalid"
        }
        VerifyError::LicenseNotValid => "LicenseNotValid",
        VerifyError::LicenseRevoked => "LicenseRevoked",
        VerifyError::AttestationSignature(_) => "SignatureInvalid",
    }
}

fn rejected(source: Option<Source>, reason: &str) -> Report {
    Report {
        source,
        authentic: false,
        reason: Some(reason.to_owned()),
        root: None,
        journal: Journal::Absent,
        details: None,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn details(verified: &Verified) -> Details {
    let a = &verified.attestation;
    let codes = |codes: &[oblivio_attestation::model::Code]| {
        codes.iter().map(|code| code.as_str().to_owned()).collect()
    };
    Details {
        id: uuid(&a.id),
        issued_at: a.issued_at,
        status: a.status.as_str().to_owned(),
        category: a.sanitization.category.as_str().to_owned(),
        method: a
            .sanitization
            .method
            .as_ref()
            .map(|code| code.as_str().to_owned()),
        verified: a.verified,
        validation: a.validation.as_ref().map(|validation| Validation {
            method: validation.method.as_str().to_owned(),
            samples: validation.samples,
            passed: validation.passed,
        }),
        disposition: a.disposition.as_ref().map(|d| d.kind.as_str().to_owned()),
        disposition_codes: a
            .disposition
            .as_ref()
            .map(|d| codes(&d.codes))
            .unwrap_or_default(),
        reserves: codes(&a.reserves),
        refusals: codes(&a.refusals),
        justification: a.justification.clone(),
        model: a.device.model.clone(),
        serial: a.device.serial.clone(),
        firmware: a.device.firmware.clone(),
        capacity_bytes: a.device.capacity_bytes,
        drive_type: a.device.drive_type.as_str().to_owned(),
        hardware_id: a
            .device
            .wwn
            .clone()
            .or_else(|| a.device.eui64.clone())
            .or_else(|| a.device.nguid.clone()),
        operator: a.operator.clone(),
        organization: a.organization.clone(),
        tool_version: a.tool.version.clone(),
        rules_version: a.tool.rules_version.clone(),
        binary_sha256: hex(&a.tool.binary_sha256),
        license_id: verified.license.id.clone(),
        customer: verified.license.customer.clone(),
    }
}

/// Verifies `input` against `trust` (roots, official first, and revocations).
pub fn check(input: &[u8], trust: &Trust) -> Report {
    if input.len() > MAX_INPUT_BYTES {
        return rejected(None, "TooLarge");
    }
    let (source, signed, journal) = if input.starts_with(b"%PDF-") {
        match attachments(input) {
            Ok(attached) => (Source::Pdf, attached.attestation, attached.journal),
            Err(ExtractError::NoAttestation) => {
                return rejected(Some(Source::Pdf), "NoAttestation");
            }
            Err(_) => return rejected(Some(Source::Pdf), "PdfUnreadable"),
        }
    } else {
        match std::str::from_utf8(input).map(str::trim) {
            Ok(text) if text.starts_with("OBV1:") => match from_qr_text(text) {
                Ok(signed) => (Source::Qr, signed, None),
                Err(error) => return rejected(Some(Source::Qr), reason(&error)),
            },
            _ => (Source::Cose, input.to_vec(), None),
        }
    };

    let mut first_error = None;
    for root in &trust.roots {
        match verify(&signed, root, &trust.revocations) {
            Ok(verified) => {
                let journal = match &journal {
                    None => Journal::Absent,
                    Some(journal) if verified.matches_journal(journal) => Journal::Matches,
                    Some(_) => Journal::Mismatch,
                };
                return Report {
                    source: Some(source),
                    authentic: true,
                    reason: None,
                    root: Some(if Some(*root) == trust.official {
                        Root::Official
                    } else {
                        Root::Development
                    }),
                    journal,
                    details: Some(details(&verified)),
                };
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    let reason = first_error.as_ref().map_or("NoTrustedRoot", reason);
    rejected(Some(source), reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oblivio_attestation::display::Lang;
    use oblivio_attestation::model::Status;
    use oblivio_attestation::to_qr_text;
    use oblivio_report::fixture::{JOURNAL, root, signed};
    use oblivio_report::{RenderOptions, render};

    fn trust(
        official: bool,
        roots: Vec<oblivio_attestation::ed25519_dalek::VerifyingKey>,
    ) -> Trust {
        Trust {
            official: if official {
                roots.first().copied()
            } else {
                None
            },
            roots,
            revocations: Vec::new(),
        }
    }

    fn roots() -> Trust {
        trust(false, vec![root().verifying_key()])
    }

    #[test]
    fn certificate_of_a_revoked_license_is_rejected() {
        let mut revoked = roots();
        revoked.revocations.push(oblivio_attestation::Revocation {
            license_id: "LIC-0001".to_owned(),
            revoked_at: 0,
        });
        let report = check(&signed(Status::Compliant, "x"), &revoked);
        assert!(!report.authentic);
        assert_eq!(report.reason.as_deref(), Some("LicenseRevoked"));
    }

    #[test]
    fn pdf_is_verified_through_its_attachments() {
        let pdf = render(
            &signed(Status::NonCompliant, "Atelier XYZ"),
            JOURNAL,
            &root().verifying_key(),
            &[],
            &RenderOptions {
                lang: Lang::En,
                logo: None,
            },
        )
        .expect("rendered");
        let report = check(&pdf, &trust(true, vec![root().verifying_key()]));
        assert!(report.authentic, "{report:?}");
        assert_eq!(report.source, Some(Source::Pdf));
        assert_eq!(report.root, Some(Root::Official));
        assert_eq!(report.journal, Journal::Matches);
        let details = report.details.expect("details");
        assert_eq!(details.status, "NON_CONFORME");
        assert_eq!(details.organization.as_deref(), Some("Atelier XYZ"));
    }

    #[test]
    fn cose_and_qr_are_verified_without_journal() {
        let cose = signed(Status::Compliant, "x");
        let report = check(&cose, &roots());
        assert!(report.authentic);
        assert_eq!(report.source, Some(Source::Cose));
        assert_eq!(report.root, Some(Root::Development));
        assert_eq!(report.journal, Journal::Absent);

        let qr = to_qr_text(&cose);
        let report = check(format!("  {qr}\n").as_bytes(), &roots());
        assert!(report.authentic);
        assert_eq!(report.source, Some(Source::Qr));
    }

    #[test]
    fn forgeries_are_rejected_without_content() {
        let mut cose = signed(Status::Compliant, "x");
        let last = cose.len() - 1;
        cose[last] ^= 1;
        let report = check(&cose, &roots());
        assert!(!report.authentic);
        assert!(report.details.is_none());
        assert!(report.reason.is_some());

        let stranger = oblivio_attestation::ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
        let report = check(
            &signed(Status::Compliant, "x"),
            &trust(false, vec![stranger.verifying_key()]),
        );
        assert_eq!(report.reason.as_deref(), Some("LicenseInvalid"));

        assert_eq!(
            check(b"OBV1:!!!", &roots()).reason.as_deref(),
            Some("Corrupted")
        );
        assert_eq!(
            check(b"%PDF-1.7 broken", &roots()).reason.as_deref(),
            Some("PdfUnreadable")
        );
    }
}
