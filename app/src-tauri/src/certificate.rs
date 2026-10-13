//! PDF certificates, rendered on the user side from the signed attestation
//! (`docs/certificat.md` §6). The app never signs: it renders what the
//! helper signed, after verifying it.

use std::path::{Path, PathBuf};

use base64::Engine;
use disphas_attestation::display::Lang;
use disphas_protocol::response::JobResult;
use disphas_report::{Logo, LogoFormat, RenderOptions, ReportError, render};

use crate::trust::Trust;

/// Folder where certificates are saved: `~/Documents/disphas`.
pub fn output_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join("Documents/disphas"))
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
    let path = dir.join(format!("disphas-{id}-{tag}.pdf"));
    std::fs::write(&path, pdf).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

/// Renders the batch certificate of `batch_id` from `files`, the signed
/// attestation and journal of each report of that batch (fetched through
/// the helper), each verified again by the renderer. Saved as
/// `disphas-lot-<batch>-<lang>.pdf`. `None` for a batch without any
/// certificate.
pub fn save_batch(
    batch_id: &str,
    files: &[(Vec<u8>, Vec<u8>)],
    lang: Lang,
    trust: &Trust,
    logo: &CustomLogo,
) -> Result<Option<PathBuf>, String> {
    use disphas_attestation::model::Batch;
    use disphas_report::batch::{BatchEntry, BatchError, render_batch};
    if !Batch::valid_id(batch_id) {
        return Err("invalid batch identifier".to_owned());
    }
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
    let path = out.join(format!("disphas-lot-{batch_id}-{tag}.pdf"));
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
        return Err("only certificates in the disphas folder can be opened".to_owned());
    }
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("xdg-open: {error}"))
}
