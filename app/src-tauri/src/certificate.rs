//! PDF certificates, rendered on the user side from the signed attestation
//! (`docs/certificat.md` §6). The app never signs: it renders what the
//! helper signed, after verifying it.

use std::path::{Path, PathBuf};

use base64::Engine;
use oblivio_attestation::display::Lang;
use oblivio_protocol::response::JobResult;
use oblivio_report::{Logo, LogoFormat, RenderOptions, ReportError, render};

use crate::trust::Trust;

/// Folder where certificates are saved: `~/Documents/Oblivio`.
pub fn output_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join("Documents/Oblivio"))
}

/// A customer logo, as loaded from the settings.
pub type CustomLogo = Option<(Vec<u8>, LogoFormat)>;

fn options(lang: Lang, logo: &CustomLogo) -> RenderOptions<'_> {
    RenderOptions {
        lang,
        logo: logo.as_ref().map(|(data, format)| Logo {
            data,
            format: *format,
        }),
    }
}

/// Renders the certificate of a finished job in `lang` and saves it.
pub fn save(result: &JobResult, lang: Lang, logo: &CustomLogo) -> Result<PathBuf, String> {
    let signed = base64::engine::general_purpose::STANDARD
        .decode(&result.attestation)
        .map_err(|error| format!("attestation: {error}"))?;
    let trust = Trust::shipped()?;
    save_signed(
        &result.job_id.0,
        &signed,
        result.journal.as_bytes(),
        lang,
        &trust,
        logo,
    )
}

/// Renders report `id` from its signed attestation and journal, and saves it.
pub fn save_signed(
    id: &str,
    signed: &[u8],
    journal: &[u8],
    lang: Lang,
    trust: &Trust,
    logo: &CustomLogo,
) -> Result<PathBuf, String> {
    // The certificate is rendered only if it chains to a trusted root.
    let options = options(lang, logo);
    let mut outcome = Err("this build trusts no root key".to_owned());
    for root in &trust.roots {
        match render(signed, journal, root, &trust.revocations, &options) {
            Ok(pdf) => {
                outcome = Ok(pdf);
                break;
            }
            Err(ReportError::Verify(error)) => outcome = Err(error.to_string()),
            Err(error) => return Err(error.to_string()),
        }
    }
    let pdf = outcome?;
    let dir = output_dir()?;
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let tag = match lang {
        Lang::En => "en",
        Lang::Fr => "fr",
        Lang::De => "de",
    };
    let path = dir.join(format!("oblivio-{id}-{tag}.pdf"));
    std::fs::write(&path, pdf).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

/// Renders the batch certificate of `batch_id` from the history: every
/// report of that batch, each verified again by the renderer. Saved as
/// `oblivio-lot-<batch>-<lang>.pdf`. `None` when the history holds no
/// verified certificate of that batch (an empty batch).
pub fn save_batch(
    batch_id: &str,
    lang: Lang,
    trust: &Trust,
    logo: &CustomLogo,
) -> Result<Option<PathBuf>, String> {
    use oblivio_attestation::model::Batch;
    use oblivio_report::batch::{BatchEntry, BatchError, render_batch};
    if !Batch::valid_id(batch_id) {
        return Err("invalid batch identifier".to_owned());
    }
    let dir = crate::history::reports_dir();
    let files: Vec<(Vec<u8>, Vec<u8>)> = crate::history::list(&dir, trust)?
        .into_iter()
        .filter(|entry| {
            entry.check == crate::history::Check::Valid
                && entry
                    .summary
                    .as_ref()
                    .is_some_and(|summary| summary.batch_id.as_deref() == Some(batch_id))
        })
        .filter_map(|entry| crate::history::read(&dir, &entry.id))
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    let entries: Vec<BatchEntry<'_>> = files
        .iter()
        .map(|(signed, journal)| BatchEntry { signed, journal })
        .collect();
    let options = options(lang, logo);
    let mut outcome = Err("this build trusts no root key".to_owned());
    for root in &trust.roots {
        match render_batch(&entries, root, &trust.revocations, &options) {
            Ok(pdf) => {
                outcome = Ok(pdf);
                break;
            }
            Err(BatchError::Drive {
                source: ReportError::Verify(error),
                index,
            }) => {
                outcome = Err(format!("drive {index}: {error}"));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    let pdf = outcome?;
    let out = output_dir()?;
    std::fs::create_dir_all(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    let tag = match lang {
        Lang::En => "en",
        Lang::Fr => "fr",
        Lang::De => "de",
    };
    let path = out.join(format!("oblivio-lot-{batch_id}-{tag}.pdf"));
    std::fs::write(&path, pdf).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Some(path))
}

/// Opens a saved certificate with the desktop's PDF viewer. Only files in
/// the certificate folder can be opened.
pub fn open(path: &Path) -> Result<(), String> {
    let dir = output_dir()?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    if !path.starts_with(&dir) || path.extension().is_none_or(|ext| ext != "pdf") {
        return Err("only certificates in the Oblivio folder can be opened".to_owned());
    }
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("xdg-open: {error}"))
}
