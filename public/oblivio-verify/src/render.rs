//! Human-readable report of a verification.

use std::fmt::Write;

use oblivio_attestation::model::{Category, Code, DispositionKind, Status, ValidationMethod};
use oblivio_attestation::pdf::ExtractError as PdfError;
use oblivio_attestation::{Verified, VerifyError};

use oblivio_attestation::display::{utc, uuid};

use crate::labels::{self, Lang, Msg};

/// Result of the journal check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalCheck {
    /// Whether its SHA-256 is the one signed.
    pub matches: bool,
    /// Whether it came from the PDF attachments rather than `--journal`.
    pub from_pdf: bool,
}

/// Report of an authentic attestation. `journal` is the result of the
/// journal check, if a journal was given or attached.
pub fn authentic(
    verified: &Verified,
    journal: Option<JournalCheck>,
    unofficial_root: bool,
    lang: Lang,
) -> String {
    let m = |message| labels::msg(message).get(lang);
    let a = &verified.attestation;
    let mut out = String::new();
    let mut line = |label: Msg, value: &str| {
        let _ = writeln!(out, "  {:<22} {value}", format!("{}:", m(label)));
    };

    line(Msg::Status, &status(a.status, lang));
    line(Msg::Report, &uuid(&a.id));
    line(Msg::Date, &utc(a.issued_at));

    line(Msg::Drive, "");
    line(Msg::Model, &a.device.model);
    line(Msg::Serial, &a.device.serial);
    line(Msg::Firmware, &a.device.firmware);
    line(
        Msg::Capacity,
        &format!(
            "{} {} ({:.1} GB)",
            a.device.capacity_bytes,
            m(Msg::Bytes),
            a.device.capacity_bytes as f64 / 1e9
        ),
    );
    let identifiers: Vec<String> = [
        ("WWN", &a.device.wwn),
        ("EUI-64", &a.device.eui64),
        ("NGUID", &a.device.nguid),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.as_ref().map(|value| format!("{name} {value}")))
    .collect();
    let identifiers = if identifiers.is_empty() {
        m(Msg::None).to_owned()
    } else {
        identifiers.join(", ")
    };
    line(
        Msg::Identifiers,
        &format!("{} — {identifiers}", a.device.drive_type.as_str()),
    );

    line(Msg::Sanitization, "");
    line(Msg::Category, category(a.sanitization.category));
    line(
        Msg::Method,
        &a.sanitization
            .method
            .as_ref()
            .map_or_else(|| m(Msg::None).to_owned(), |method| labelled(method, lang)),
    );
    line(Msg::Standards, &a.sanitization.references.join(", "));
    line(
        Msg::Verification,
        m(match a.verified {
            Some(true) => Msg::Passed,
            Some(false) => Msg::Failed,
            None => Msg::NotRun,
        }),
    );
    let validation = a.validation.as_ref().map_or_else(
        || m(Msg::NotRun).to_owned(),
        |validation| {
            format!(
                "{}, {} {}, {}",
                m(match validation.method {
                    ValidationMethod::Canaries => Msg::Canaries,
                    ValidationMethod::Pattern => Msg::Pattern,
                }),
                validation.samples,
                m(Msg::Samples),
                m(if validation.passed {
                    Msg::Passed
                } else {
                    Msg::Failed
                })
            )
        },
    );
    line(Msg::Validation, &validation);

    if let Some(disposition) = &a.disposition {
        let kind = m(match disposition.kind {
            DispositionKind::Reuse => Msg::Reuse,
            DispositionKind::DestroyRecommended => Msg::DestroyRecommended,
        });
        line(Msg::Future, kind);
        for code in &disposition.codes {
            line_item(&mut out, code, lang);
        }
    }
    if !a.refusals.is_empty() {
        let _ = writeln!(out, "  {}:", m(Msg::Refusals));
        for code in &a.refusals {
            line_item(&mut out, code, lang);
        }
    }
    let _ = writeln!(
        out,
        "  {:<22} {}",
        format!("{}:", m(Msg::Reserves)),
        if a.reserves.is_empty() {
            m(Msg::None)
        } else {
            ""
        }
    );
    for code in &a.reserves {
        line_item(&mut out, code, lang);
    }

    let mut line = |label: Msg, value: &str| {
        let _ = writeln!(out, "  {:<22} {value}", format!("{}:", m(label)));
    };
    if let Some(justification) = &a.justification {
        line(Msg::Justification, justification);
    }
    if let Some(organization) = &a.organization {
        line(Msg::Organization, organization);
    }
    if let Some(operator) = &a.operator {
        line(Msg::Operator, operator);
    }
    line(
        Msg::Tool,
        &format!("Oblivio {}, rules {}", a.tool.version, a.tool.rules_version),
    );
    let license = &verified.license;
    line(
        Msg::License,
        &format!("{} — {} ({})", license.id, license.customer, license.offer),
    );
    if let Some(check) = journal {
        let verdict = m(if check.matches {
            Msg::JournalMatches
        } else {
            Msg::JournalMismatch
        });
        if check.from_pdf {
            line(
                Msg::Journal,
                &format!("{verdict} ({})", m(Msg::JournalFromPdf)),
            );
        } else {
            line(Msg::Journal, verdict);
        }
    }

    let mut header = format!("✔ {}\n", m(Msg::Authentic));
    if unofficial_root {
        header.push_str(&format!("⚠ {}\n", m(Msg::UnofficialRoot)));
    }
    header + "\n" + &out
}

/// Report of a PDF whose attestation cannot be read.
pub fn rejected_pdf(error: &PdfError, lang: Lang) -> String {
    let reason = match error {
        PdfError::NoAttestation => Msg::ErrPdfNoAttestation,
        PdfError::NotPdf(_) | PdfError::Duplicate(_) => Msg::ErrPdfUnreadable,
    };
    format!(
        "✘ {}: {}\n  ({error})\n",
        labels::msg(Msg::NotAuthentic).get(lang),
        labels::msg(reason).get(lang)
    )
}

/// Report of a rejected attestation.
pub fn rejected(error: &VerifyError, lang: Lang) -> String {
    let reason = match error {
        VerifyError::NotOblivio => Msg::ErrNotOblivio,
        VerifyError::Corrupted => Msg::ErrCorrupted,
        VerifyError::AttestationEnvelope(_) | VerifyError::AttestationContent(_) => {
            Msg::ErrMalformed
        }
        VerifyError::License(_) | VerifyError::LicenseContent(_) | VerifyError::WorkstationKey => {
            Msg::ErrLicense
        }
        VerifyError::LicenseNotValid => Msg::ErrLicenseNotValid,
        VerifyError::LicenseRevoked => Msg::ErrLicenseRevoked,
        VerifyError::AttestationSignature(_) => Msg::ErrSignature,
    };
    format!(
        "✘ {}: {}\n  ({error})\n",
        labels::msg(Msg::NotAuthentic).get(lang),
        labels::msg(reason).get(lang)
    )
}

fn status(status: Status, lang: Lang) -> String {
    let m = |message| labels::msg(message).get(lang);
    match status {
        Status::Compliant => m(Msg::Compliant).to_owned(),
        Status::NonCompliant => {
            format!("{} — {}", m(Msg::NonCompliant), m(Msg::DestructionRequired))
        }
        Status::Refused => format!("{} — {}", m(Msg::Refused), m(Msg::DestructionRequired)),
    }
}

/// Never translated.
fn category(category: Category) -> &'static str {
    match category {
        Category::Clear => "Clear",
        Category::Purge => "Purge",
        Category::None => "—",
    }
}

fn labelled(code: &Code, lang: Lang) -> String {
    match labels::code(code.as_str()) {
        Some(label) => format!("{} ({})", label.get(lang), code.as_str()),
        None => format!(
            "{} ({})",
            code.as_str(),
            labels::msg(Msg::NoLabel).get(lang)
        ),
    }
}

fn line_item(out: &mut String, code: &Code, lang: Lang) {
    let _ = writeln!(out, "    - {}", labelled(code, lang));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_codes_are_shown_as_is() {
        let code = Code::new("Z99").expect("valid code");
        assert_eq!(
            labelled(&code, Lang::Fr),
            "Z99 (pas de libellé dans cette version)"
        );
    }
}
