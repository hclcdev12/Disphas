//! Certificate rendering: PDF/A-3b, QR code and EN/FR/DE translations
//! (Project Fluent), laid out by Typst (`docs/certificat.md` §6).
//!
//! The PDF is only a rendering of the signed attestation: it is rendered
//! from the signed bytes after verifying them, and can be regenerated in
//! another language without a new signature. The attestation and the
//! journal are attached to the PDF.
//!
//! This crate never performs low-level disk access, never handles a private
//! key, and makes no network access: Typst package downloads are disabled.

pub mod extract;
mod i18n;
mod qr;

/// Fictional signed attestations with fixed test keys, for tests and examples.
/// Never compiled into the product: behind the `fixtures` feature.
#[cfg(any(test, feature = "fixtures"))]
pub mod fixture;

use oblivio_attestation::display::{self, Lang};
use oblivio_attestation::ed25519_dalek::VerifyingKey;
use oblivio_attestation::model::{Category, Code, DispositionKind, Status, ValidationMethod};
use oblivio_attestation::{Revocation, VerifyError, to_qr_text, verify};
use sha2::{Digest, Sha256};
use typst::foundations::{Array, Bytes, Datetime, Dict, IntoValue, Smart, Value};
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

use crate::i18n::Translator;

const TEMPLATE: &str = include_str!("certificate.typ");

/// Oblivio wordmark for white backgrounds, used when no custom logo is given.
const WORDMARK: &[u8] = include_bytes!("../../../assets/logo/oblivio-wordmark-light.svg");
/// Secondary mark printed next to the QR code (`docs/oblivio_charte_graphique.md`).
const STAMP: &[u8] = include_bytes!("../../../assets/logo/oblivio-stamp.svg");

/// Fonts of the brand charter, embedded so rendering is offline and reproducible.
const FONTS: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/Inter-Regular-latin.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Regular-latin-ext.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Bold-latin.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Bold-latin-ext.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular-latin.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular-latin-ext.ttf"),
];

/// Why a certificate cannot be rendered.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// The attestation does not verify: nothing is rendered.
    #[error("attestation not authentic: {0}")]
    Verify(#[from] VerifyError),
    /// The journal is not the one the attestation refers to.
    #[error("the journal does not match the attestation")]
    JournalMismatch,
    /// A translation is missing or broken.
    #[error("translation: {0}")]
    Translation(String),
    /// The QR code cannot be built.
    #[error("QR code: {0}")]
    Qr(String),
    /// The layout failed.
    #[error("layout: {0}")]
    Layout(String),
    /// The PDF export failed, for instance on a PDF/A violation.
    #[error("PDF export: {0}")]
    Pdf(String),
}

/// Image format of a custom logo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoFormat {
    /// PNG.
    Png,
    /// SVG.
    Svg,
}

/// A custom logo (Atelier offer).
#[derive(Debug, Clone, Copy)]
pub struct Logo<'a> {
    /// Image data.
    pub data: &'a [u8],
    /// Its format.
    pub format: LogoFormat,
}

/// Rendering choices.
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions<'a> {
    /// Language of the certificate.
    pub lang: Lang,
    /// Custom logo instead of the Oblivio wordmark.
    pub logo: Option<Logo<'a>>,
}

/// Renders the PDF/A-3b certificate of a signed attestation.
///
/// The attestation is verified against `root` first and the journal must
/// match it: an attestation that does not verify is never rendered.
/// The output is deterministic: same inputs, same bytes.
pub fn render(
    signed: &[u8],
    journal: &[u8],
    root: &VerifyingKey,
    revocations: &[Revocation],
    options: &RenderOptions<'_>,
) -> Result<Vec<u8>, ReportError> {
    let verified = verify(signed, root, revocations)?;
    if !verified.matches_journal(journal) {
        return Err(ReportError::JournalMismatch);
    }
    let translator = Translator::new(options.lang)?;
    let inputs = inputs(&verified, signed, journal, options, &translator)?;
    compile_pdf(
        TEMPLATE,
        inputs,
        verified.attestation.issued_at,
        &display::uuid(&verified.attestation.id),
    )
}

fn compile_pdf(
    template: &str,
    inputs: Dict,
    issued_at: u64,
    ident: &str,
) -> Result<Vec<u8>, ReportError> {
    let engine = TypstEngine::builder()
        .main_file(template)
        .fonts(FONTS.iter().copied())
        .build();
    let document: PagedDocument = engine
        .compile_with_input(inputs)
        .output
        .map_err(|error| ReportError::Layout(format!("{error:?}")))?;
    let options = PdfOptions {
        ident: Smart::Custom(ident.to_owned()),
        timestamp: Some(Timestamp::new_utc(datetime(issued_at)?)),
        standards: PdfStandards::new(&[PdfStandard::A_3b])
            .map_err(|error| ReportError::Pdf(format!("{error:?}")))?,
        ..PdfOptions::default()
    };
    typst_pdf::pdf(&document, &options).map_err(|error| ReportError::Pdf(format!("{error:?}")))
}

fn datetime(epoch: u64) -> Result<Datetime, ReportError> {
    let (year, month, day, hour, minute, second) = display::civil(epoch);
    let narrow = |value: u64| u8::try_from(value).ok();
    i32::try_from(year)
        .ok()
        .zip(narrow(month).zip(narrow(day)))
        .and_then(|(year, (month, day))| {
            Datetime::from_ymd_hms(
                year,
                month,
                day,
                narrow(hour)?,
                narrow(minute)?,
                narrow(second)?,
            )
        })
        .ok_or_else(|| ReportError::Layout(format!("date out of range: {epoch}")))
}

// --- Template data.

fn row(label: String, values: Vec<String>, mono: bool) -> Value {
    let mut dict = Dict::new();
    dict.insert("label".into(), label.into_value());
    dict.insert(
        "value".into(),
        values
            .into_iter()
            .map(IntoValue::into_value)
            .collect::<Array>()
            .into_value(),
    );
    dict.insert("mono".into(), mono.into_value());
    dict.into_value()
}

fn section(title: String, rows: Vec<Value>) -> Value {
    let mut dict = Dict::new();
    dict.insert("title".into(), title.into_value());
    dict.insert(
        "rows".into(),
        rows.into_iter().collect::<Array>().into_value(),
    );
    dict.into_value()
}

fn banner(ok: bool, title: String, subtitle: Option<String>) -> Value {
    let mut dict = Dict::new();
    dict.insert("kind".into(), if ok { "ok" } else { "alert" }.into_value());
    dict.insert("title".into(), title.into_value());
    dict.insert(
        "subtitle".into(),
        subtitle.map_or(Value::None, IntoValue::into_value),
    );
    dict.into_value()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Hexadecimal in groups of four, so long values can wrap.
fn grouped_hex(bytes: &[u8]) -> String {
    bytes.chunks(2).map(hex).collect::<Vec<_>>().join(" ")
}

/// Label of a code in the certificate language, the code itself in brackets.
fn code_label(code: &Code, lang: Lang) -> String {
    match display::code(code.as_str()) {
        Some(label) => format!("{} ({})", label.get(lang), code.as_str()),
        None => code.as_str().to_owned(),
    }
}

fn inputs(
    verified: &oblivio_attestation::Verified,
    signed: &[u8],
    journal: &[u8],
    options: &RenderOptions<'_>,
    t: &Translator,
) -> Result<Dict, ReportError> {
    let a = &verified.attestation;
    let lang = t.lang();
    let codes = |codes: &[Code]| -> Result<Vec<String>, ReportError> {
        if codes.is_empty() {
            Ok(vec![t.text("none")?])
        } else {
            Ok(codes.iter().map(|code| code_label(code, lang)).collect())
        }
    };

    // Banners.
    let mut banners = Vec::new();
    match a.status {
        Status::Compliant => banners.push(banner(
            true,
            t.text("status-compliant")?,
            Some(t.text("banner-compliant")?),
        )),
        Status::NonCompliant => banners.push(banner(
            false,
            t.text("status-non-compliant")?,
            Some(t.text("banner-non-compliant")?),
        )),
        Status::Refused => banners.push(banner(
            false,
            t.text("status-refused")?,
            Some(t.text("banner-refused")?),
        )),
    }
    if a.status == Status::Compliant
        && a.disposition
            .as_ref()
            .is_some_and(|disposition| disposition.kind == DispositionKind::DestroyRecommended)
    {
        banners.push(banner(false, t.text("banner-destroy-recommended")?, None));
    }

    // Drive.
    let device = &a.device;
    let identifiers: Vec<String> = [
        ("WWN", &device.wwn),
        ("EUI-64", &device.eui64),
        ("NGUID", &device.nguid),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.as_ref().map(|value| format!("{name} {value}")))
    .collect();
    let drive = section(
        t.text("section-drive")?,
        vec![
            row(t.text("model")?, vec![device.model.clone()], false),
            row(t.text("serial")?, vec![device.serial.clone()], true),
            row(t.text("firmware")?, vec![device.firmware.clone()], true),
            row(
                t.text("capacity")?,
                vec![t.with(
                    "capacity-value",
                    &[
                        ("bytes", t.integer(device.capacity_bytes)),
                        ("gb", t.decimal(device.capacity_bytes / 100_000_000)),
                    ],
                )?],
                false,
            ),
            row(
                t.text("drive-type")?,
                vec![device.drive_type.as_str().to_owned()],
                false,
            ),
            if identifiers.is_empty() {
                row(t.text("identifiers")?, vec![t.text("none")?], false)
            } else {
                row(t.text("identifiers")?, identifiers, true)
            },
        ],
    );

    // Sanitization.
    let san = &a.sanitization;
    let category = match san.category {
        Category::Clear => "Clear",
        Category::Purge => "Purge",
        Category::None => "—",
    };
    let method = match &san.method {
        Some(method) => code_label(method, lang),
        None => t.text("none")?,
    };
    let verification = t.text(match a.verified {
        Some(true) => "passed",
        Some(false) => "failed",
        None => "not-run",
    })?;
    let validation = match &a.validation {
        Some(validation) => t.with(
            "validation-value",
            &[
                (
                    "method",
                    t.text(match validation.method {
                        ValidationMethod::Canaries => "canaries",
                        ValidationMethod::Pattern => "pattern",
                    })?,
                ),
                ("samples", t.integer(u64::from(validation.samples))),
                (
                    "result",
                    t.text(if validation.passed {
                        "passed"
                    } else {
                        "failed"
                    })?,
                ),
            ],
        )?,
        None => t.text("not-run")?,
    };
    let sanitization = section(
        t.text("section-sanitization")?,
        vec![
            row(t.text("category")?, vec![category.to_owned()], false),
            row(t.text("method")?, vec![method], false),
            row(t.text("standards")?, vec![san.references.join(", ")], false),
            row(t.text("verification")?, vec![verification], false),
            row(t.text("validation")?, vec![validation], false),
        ],
    );

    // Future and reserves.
    let mut future_rows = Vec::new();
    if let Some(disposition) = &a.disposition {
        let mut values = vec![t.text(match disposition.kind {
            DispositionKind::Reuse => "reuse",
            DispositionKind::DestroyRecommended => "destroy-recommended",
        })?];
        values.extend(disposition.codes.iter().map(|code| code_label(code, lang)));
        future_rows.push(row(t.text("future")?, values, false));
    }
    if !a.refusals.is_empty() {
        future_rows.push(row(t.text("refusals")?, codes(&a.refusals)?, false));
    }
    future_rows.push(row(t.text("reserves")?, codes(&a.reserves)?, false));
    let future = section(t.text("section-future")?, future_rows);

    // Issuer.
    let license = &verified.license;
    let mut issuer_rows = Vec::new();
    if let Some(organization) = &a.organization {
        issuer_rows.push(row(
            t.text("organization")?,
            vec![organization.clone()],
            false,
        ));
    }
    if let Some(operator) = &a.operator {
        issuer_rows.push(row(t.text("operator")?, vec![operator.clone()], false));
    }
    issuer_rows.push(row(
        t.text("tool")?,
        vec![format!(
            "Oblivio {} · rules {}",
            a.tool.version, a.tool.rules_version
        )],
        false,
    ));
    issuer_rows.push(row(
        t.text("license")?,
        vec![format!(
            "{} — {} ({})",
            license.id, license.customer, license.offer
        )],
        false,
    ));
    let mut issuer = Dict::new();
    issuer.insert("title".into(), t.text("section-issuer")?.into_value());
    issuer.insert(
        "rows".into(),
        issuer_rows.into_iter().collect::<Array>().into_value(),
    );

    // Annex.
    let attestation_digest = Sha256::digest(signed);
    let attestation_hash = grouped_hex(&attestation_digest);
    let mut annex_rows = vec![row(t.text("report")?, vec![display::uuid(&a.id)], true)];
    if let Some(validation) = &a.validation {
        annex_rows.push(row(
            t.text("seed")?,
            vec![grouped_hex(&validation.seed)],
            true,
        ));
    }
    annex_rows.extend([
        row(
            t.text("rules-version")?,
            vec![a.tool.rules_version.clone()],
            true,
        ),
        row(
            t.text("binary-hash")?,
            vec![grouped_hex(&a.tool.binary_sha256)],
            true,
        ),
        row(
            t.text("journal-hash")?,
            vec![grouped_hex(&a.journal_sha256)],
            true,
        ),
        row(
            t.text("attestation-hash")?,
            vec![attestation_hash.clone()],
            true,
        ),
        row(
            t.text("attachments")?,
            vec![t.text("attachments-value")?],
            false,
        ),
    ]);
    let mut annex = Dict::new();
    annex.insert("title".into(), t.text("annex")?.into_value());
    annex.insert("intro".into(), t.text("annex-intro")?.into_value());
    annex.insert(
        "rows".into(),
        annex_rows.into_iter().collect::<Array>().into_value(),
    );

    // Assemble.
    let (year, month, day, hour, minute, second) = display::civil(a.issued_at);
    let mut inputs = Dict::new();
    let mut set = |key: &str, value: Value| {
        inputs.insert(key.into(), value);
    };
    set(
        "lang",
        match lang {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::De => "de",
        }
        .into_value(),
    );
    set("title", t.text("title")?.into_value());
    set("report_label", t.text("report")?.into_value());
    set("report_id", display::uuid(&a.id).into_value());
    set("issued_label", t.text("issued")?.into_value());
    set("issued", display::utc(a.issued_at).into_value());
    for (key, value) in [
        ("year", year),
        ("month", month),
        ("day", day),
        ("hour", hour),
        ("minute", minute),
        ("second", second),
    ] {
        set(key, i64::try_from(value).unwrap_or_default().into_value());
    }
    let logo = options.logo.unwrap_or(Logo {
        data: WORDMARK,
        format: LogoFormat::Svg,
    });
    set("logo", Bytes::new(logo.data.to_vec()).into_value());
    set(
        "logo_format",
        match logo.format {
            LogoFormat::Svg => "svg",
            LogoFormat::Png => "png",
        }
        .into_value(),
    );
    set("stamp", Bytes::new(STAMP.to_vec()).into_value());
    set(
        "banners",
        banners.into_iter().collect::<Array>().into_value(),
    );
    set(
        "justification",
        match &a.justification {
            Some(text) => {
                let mut dict = Dict::new();
                dict.insert("label".into(), t.text("justification")?.into_value());
                dict.insert("text".into(), text.clone().into_value());
                dict.into_value()
            }
            None => Value::None,
        },
    );
    set(
        "sections",
        [drive, sanitization, future]
            .into_iter()
            .collect::<Array>()
            .into_value(),
    );
    set("issuer", issuer.into_value());
    set("fingerprint_label", t.text("fingerprint")?.into_value());
    set(
        "fingerprint",
        grouped_hex(&attestation_digest[..16]).into_value(),
    );
    set("verify_hint", t.text("verify-hint")?.into_value());
    set(
        "qr",
        Bytes::new(qr::svg(&to_qr_text(signed))?.into_bytes()).into_value(),
    );
    set("annex", annex.into_value());
    set("legal", t.text("legal")?.into_value());
    set("attestation", Bytes::new(signed.to_vec()).into_value());
    set("journal", Bytes::new(journal.to_vec()).into_value());
    set(
        "attestation_description",
        "attestation (COSE_Sign1)".into_value(),
    );
    set("journal_description", "journal (JSON)".into_value());
    Ok(inputs)
}

#[cfg(test)]
mod tests;
