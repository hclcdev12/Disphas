//! Batch certificate: one PDF summing up the drives erased in one batch,
//! with every drive's signed attestation and journal attached
//! (`docs/certificat.md` §6.1).
//!
//! Each drive stays proven by its own attestation, which carries the batch
//! number under its signature. The batch PDF is rendered only when every
//! attestation verifies, every journal matches, and all of them name the
//! same batch.

use oblivio_attestation::display::{self, Lang};
use oblivio_attestation::ed25519_dalek::VerifyingKey;
use oblivio_attestation::model::{Batch, Category, Status};
use oblivio_attestation::{Revocation, Verified, verify};
use typst::foundations::{Array, Bytes, Dict, IntoValue, Value};

use crate::i18n::Translator;
use crate::{Logo, LogoFormat, RenderOptions, ReportError, STAMP, WORDMARK, compile_pdf};

const TEMPLATE: &str = include_str!("batch.typ");

/// Most drives in one batch certificate.
pub const MAX_DRIVES: usize = 2000;

/// One drive of the batch: its signed attestation and its journal.
#[derive(Debug, Clone, Copy)]
pub struct BatchEntry<'a> {
    /// Signed attestation (COSE_Sign1).
    pub signed: &'a [u8],
    /// Journal the attestation refers to.
    pub journal: &'a [u8],
}

/// Why a batch certificate is refused.
#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    /// No drive, or more than [`MAX_DRIVES`].
    #[error("a batch certificate needs 1 to {MAX_DRIVES} drives")]
    Size,
    /// An attestation does not render (not authentic, wrong journal…).
    #[error("drive {index}: {source}")]
    Drive {
        /// Position of the drive in the input, from 1.
        index: usize,
        /// Why it does not render.
        source: ReportError,
    },
    /// An attestation carries no batch number.
    #[error("drive {0} was not erased in a batch")]
    NoBatch(usize),
    /// Attestations name different batches.
    #[error("drive {0} belongs to another batch")]
    MixedBatches(usize),
    /// Attestations signed under licenses of different customers.
    #[error("drive {0} is licensed to another customer")]
    MixedCustomers(usize),
    /// The same attestation is given twice.
    #[error("drive {0} is given twice")]
    Duplicate(usize),
    /// Rendering failed.
    #[error(transparent)]
    Render(#[from] ReportError),
}

struct Drive<'a> {
    verified: Verified,
    entry: BatchEntry<'a>,
}

/// Renders the PDF/A-3b batch certificate of `entries`, all verified
/// against `root`. Drives are listed in erasure order. Deterministic.
pub fn render_batch(
    entries: &[BatchEntry<'_>],
    root: &VerifyingKey,
    revocations: &[Revocation],
    options: &RenderOptions<'_>,
) -> Result<Vec<u8>, BatchError> {
    if entries.is_empty() || entries.len() > MAX_DRIVES {
        return Err(BatchError::Size);
    }
    let mut drives = Vec::with_capacity(entries.len());
    let mut batch: Option<Batch> = None;
    for (index, entry) in entries.iter().enumerate() {
        let number = index + 1;
        let verified =
            verify(entry.signed, root, revocations).map_err(|error| BatchError::Drive {
                index: number,
                source: ReportError::Verify(error),
            })?;
        if !verified.matches_journal(entry.journal) {
            return Err(BatchError::Drive {
                index: number,
                source: ReportError::JournalMismatch,
            });
        }
        let Some(own) = &verified.attestation.batch else {
            return Err(BatchError::NoBatch(number));
        };
        match &batch {
            None => batch = Some(own.clone()),
            Some(first) if first != own => return Err(BatchError::MixedBatches(number)),
            Some(_) => {}
        }
        if drives.first().is_some_and(|first: &Drive<'_>| {
            first.verified.license.customer != verified.license.customer
        }) {
            return Err(BatchError::MixedCustomers(number));
        }
        if drives
            .iter()
            .any(|drive: &Drive<'_>| drive.verified.attestation.id == verified.attestation.id)
        {
            return Err(BatchError::Duplicate(number));
        }
        drives.push(Drive {
            verified,
            entry: *entry,
        });
    }
    let Some(batch) = batch else {
        return Err(BatchError::Size);
    };
    drives.sort_by_key(|drive| {
        (
            drive.verified.attestation.issued_at,
            drive.verified.attestation.id,
        )
    });

    let translator = Translator::new(options.lang)?;
    let last = drives
        .iter()
        .map(|drive| drive.verified.attestation.issued_at)
        .max()
        .unwrap_or_default();
    let inputs = inputs(&batch, &drives, options, &translator)?;
    Ok(compile_pdf(TEMPLATE, inputs, last, &batch.id)?)
}

fn text_fact(label: String, value: String) -> Value {
    let mut dict = Dict::new();
    dict.insert("label".into(), label.into_value());
    dict.insert("value".into(), value.into_value());
    dict.into_value()
}

fn distinct(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    for value in values {
        if !list.contains(&value) {
            list.push(value);
        }
    }
    list
}

fn inputs(
    batch: &Batch,
    drives: &[Drive<'_>],
    options: &RenderOptions<'_>,
    t: &Translator,
) -> Result<Dict, ReportError> {
    let lang = t.lang();
    let first = drives
        .first()
        .map_or(0, |d| d.verified.attestation.issued_at);
    let last = drives
        .last()
        .map_or(0, |d| d.verified.attestation.issued_at);
    let count = |status: Status| {
        drives
            .iter()
            .filter(|d| d.verified.attestation.status == status)
            .count() as u64
    };
    let (compliant, non_compliant, refused) = (
        count(Status::Compliant),
        count(Status::NonCompliant),
        count(Status::Refused),
    );
    let capacity: u64 = drives
        .iter()
        .map(|d| d.verified.attestation.device.capacity_bytes)
        .sum();

    let mut facts = vec![
        text_fact(t.text("batch-drives")?, t.integer(drives.len() as u64)),
        text_fact(
            t.text("batch-capacity")?,
            t.with(
                "capacity-short",
                &[("gb", t.decimal(capacity / 100_000_000))],
            )?,
        ),
    ];
    let organizations = distinct(
        drives
            .iter()
            .filter_map(|d| d.verified.attestation.organization.clone()),
    );
    if !organizations.is_empty() {
        facts.push(text_fact(t.text("organization")?, organizations.join(", ")));
    }
    let operators = distinct(
        drives
            .iter()
            .filter_map(|d| d.verified.attestation.operator.clone()),
    );
    if !operators.is_empty() {
        facts.push(text_fact(t.text("operator")?, operators.join(", ")));
    }
    let licenses = distinct(drives.iter().map(|d| {
        let license = &d.verified.license;
        format!("{} — {} ({})", license.id, license.customer, license.offer)
    }));
    facts.push(text_fact(t.text("license")?, licenses.join(" · ")));
    let tools = distinct(drives.iter().map(|d| {
        let tool = &d.verified.attestation.tool;
        format!("Oblivio {} · rules {}", tool.version, tool.rules_version)
    }));
    facts.push(text_fact(t.text("tool")?, tools.join(" · ")));

    let mut rows = Array::new();
    let mut files = Array::new();
    for (index, drive) in drives.iter().enumerate() {
        let a = &drive.verified.attestation;
        let report = display::uuid(&a.id);
        let mut row = Dict::new();
        let mut put = |key: &str, value: Value| {
            row.insert(key.into(), value);
        };
        put("index", (index + 1).to_string().into_value());
        put("serial", a.device.serial.clone().into_value());
        // Hardware identifier under the serial number: WWN on SATA drives,
        // EUI-64 or NGUID on NVMe ones, whichever is present first.
        let hardware = [
            ("WWN", &a.device.wwn),
            ("EUI-64", &a.device.eui64),
            ("NGUID", &a.device.nguid),
        ]
        .into_iter()
        .find_map(|(name, value)| value.as_ref().map(|value| format!("{name} {value}")));
        put(
            "hardware",
            hardware.map_or(Value::None, IntoValue::into_value),
        );
        put("model", a.device.model.clone().into_value());
        put(
            "capacity",
            t.with(
                "capacity-short",
                &[("gb", t.decimal(a.device.capacity_bytes / 100_000_000))],
            )?
            .into_value(),
        );
        put(
            "method",
            // Label alone: the table is narrow, the code is in the attestation.
            match &a.sanitization.method {
                Some(method) => display::code(method.as_str()).map_or_else(
                    || method.as_str().to_owned(),
                    |label| label.get(lang).to_owned(),
                ),
                None => t.text("none")?,
            }
            .into_value(),
        );
        put(
            "category",
            match a.sanitization.category {
                Category::Clear => "Clear",
                Category::Purge => "Purge",
                Category::None => "—",
            }
            .into_value(),
        );
        put(
            "status",
            t.text(match a.status {
                Status::Compliant => "status-compliant",
                Status::NonCompliant => "status-non-compliant",
                Status::Refused => "status-refused",
            })?
            .into_value(),
        );
        put("compliant", (a.status == Status::Compliant).into_value());
        put("report", report.clone().into_value());
        rows.push(row.into_value());

        for (name, data, relationship, mime, description) in [
            (
                format!("{report}.cose"),
                drive.entry.signed,
                "source",
                "application/cose",
                "attestation (COSE_Sign1)",
            ),
            (
                format!("{report}.json"),
                drive.entry.journal,
                "supplement",
                "application/json",
                "journal (JSON)",
            ),
        ] {
            let mut file = Dict::new();
            file.insert("name".into(), name.into_value());
            file.insert("data".into(), Bytes::new(data.to_vec()).into_value());
            file.insert("relationship".into(), relationship.into_value());
            file.insert("mime".into(), mime.into_value());
            file.insert("description".into(), description.into_value());
            files.push(file.into_value());
        }
    }

    let headers: Array = [
        "#",
        &t.text("serial")?,
        &t.text("model")?,
        &t.text("capacity")?,
        &t.text("method")?,
        &t.text("category")?,
        &t.text("batch-status")?,
        &t.text("report")?,
    ]
    .into_iter()
    .map(|header| header.to_owned().into_value())
    .collect();

    let (year, month, day, hour, minute, second) = display::civil(last);
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
    set("title", t.text("batch-title")?.into_value());
    set("batch_label_label", t.text("batch")?.into_value());
    set("batch_id", batch.id.clone().into_value());
    set(
        "batch_label",
        batch
            .label
            .clone()
            .map_or(Value::None, IntoValue::into_value),
    );
    set("period_label", t.text("batch-period")?.into_value());
    set(
        "period",
        if first == last {
            display::utc(first)
        } else {
            format!("{} — {}", display::utc(first), display::utc(last))
        }
        .into_value(),
    );
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
    set(
        "all_compliant",
        (compliant == drives.len() as u64).into_value(),
    );
    set("summary_title", t.text("batch-summary-title")?.into_value());
    set(
        "summary",
        t.with(
            "batch-summary",
            &[
                ("compliant", t.integer(compliant)),
                ("noncompliant", t.integer(non_compliant)),
                ("refused", t.integer(refused)),
            ],
        )?
        .into_value(),
    );
    set("facts", facts.into_iter().collect::<Array>().into_value());
    set("drives_title", t.text("batch-drives-title")?.into_value());
    set("headers", headers.into_value());
    set("rows", rows.into_value());
    set("files", files.into_value());
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
    set("verify_hint", t.text("batch-verify-hint")?.into_value());
    set("legal", t.text("legal")?.into_value());
    Ok(inputs)
}
