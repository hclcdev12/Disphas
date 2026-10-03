//! Human-readable report of a verification.

use std::fmt::Write;

use oblivio_attestation::model::{Category, Code, DispositionKind, Status, ValidationMethod};
use oblivio_attestation::{Verified, VerifyError};

use crate::labels::{self, Lang, Msg};

/// Report of an authentic attestation. `journal` is the result of the
/// journal check, if a journal was given.
pub fn authentic(
    verified: &Verified,
    journal: Option<bool>,
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
    if let Some(matches) = journal {
        line(
            Msg::Journal,
            m(if matches {
                Msg::JournalMatches
            } else {
                Msg::JournalMismatch
            }),
        );
    }

    let mut header = format!("✔ {}\n", m(Msg::Authentic));
    if unofficial_root {
        header.push_str(&format!("⚠ {}\n", m(Msg::UnofficialRoot)));
    }
    header + "\n" + &out
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

/// UUID in its canonical 8-4-4-4-12 form.
pub fn uuid(id: &[u8; 16]) -> String {
    let hex: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// UTC date and time of an epoch timestamp, `YYYY-MM-DD HH:MM:SS UTC`.
pub fn utc(epoch: u64) -> String {
    let days = epoch / 86_400;
    let seconds = epoch % 86_400;
    // Civil date from days since 1970-01-01 (H. Hinnant, "chrono-compatible
    // low-level date algorithms"), for non-negative days.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_dates() {
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(utc(1_791_158_400), "2026-10-05 00:00:00 UTC");
        assert_eq!(utc(4_107_542_399), "2100-02-28 23:59:59 UTC");
    }

    #[test]
    fn formats_uuids() {
        let id = [
            0x01, 0x92, 0x8c, 0x3e, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0, 0, 0, 0, 0, 1,
        ];
        assert_eq!(uuid(&id), "01928c3e-0000-7000-8000-000000000001");
    }

    #[test]
    fn unknown_codes_are_shown_as_is() {
        let code = Code::new("Z99").expect("valid code");
        assert_eq!(
            labelled(&code, Lang::Fr),
            "Z99 (pas de libellé dans cette version)"
        );
    }
}
