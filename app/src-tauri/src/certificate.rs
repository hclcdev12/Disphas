//! PDF certificates, rendered on the user side from the signed attestation
//! (`docs/certificat.md` §6). The app never signs: it renders what the
//! helper signed, after verifying it.

use std::path::{Path, PathBuf};

use base64::Engine;
use oblivio_attestation::display::Lang;
use oblivio_protocol::response::JobResult;
use oblivio_report::{RenderOptions, ReportError, render};

/// Folder where certificates are saved: `~/Documents/Oblivio`.
pub fn output_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join("Documents/Oblivio"))
}

/// Renders the certificate of a finished job in `lang` and saves it.
pub fn save(result: &JobResult, lang: Lang) -> Result<PathBuf, String> {
    let signed = base64::engine::general_purpose::STANDARD
        .decode(&result.attestation)
        .map_err(|error| format!("attestation: {error}"))?;
    // The certificate is rendered only if it chains to a trusted root.
    let options = RenderOptions { lang, logo: None };
    let mut outcome = Err("this build trusts no root key".to_owned());
    for root in oblivio_license::trusted_roots() {
        match render(&signed, result.journal.as_bytes(), &root, &[], &options) {
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
    let path = dir.join(format!("oblivio-{}-{tag}.pdf", result.job_id.0));
    std::fs::write(&path, pdf).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
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
