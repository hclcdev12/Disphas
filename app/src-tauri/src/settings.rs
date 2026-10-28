//! Operator settings, per user account: the organization and the operator
//! written into certificates, and the customer's logo printed on the PDFs
//! (Atelier offer). Kept in the app configuration folder
//! (`~/.config/org.disphas.app`): `settings.json`, `logo.png` or `logo.svg`.
//!
//! Bounds match the helper's, which checks again before signing.

use std::path::{Path, PathBuf};

use disphas_report::LogoFormat;
use serde::{Deserialize, Serialize};

/// Longest operator name or identifier, as accepted by the helper.
pub const MAX_OPERATOR_CHARS: usize = 64;
/// Longest organization name, as accepted by the helper.
pub const MAX_ORGANIZATION_CHARS: usize = 128;
/// Largest logo file.
pub const MAX_LOGO_BYTES: usize = 1 << 20;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Stored settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Organization written into certificates.
    pub organization: Option<String>,
    /// Operator name or identifier written into certificates.
    pub operator: Option<String>,
    /// Format of the stored logo: `png` or `svg`.
    pub logo: Option<String>,
    /// Strict mode: the confirmation of each erasure also asks for the
    /// serial number read on the drive, scanned or typed
    /// (`docs/cadre_projet.md` §5.4).
    #[serde(default)]
    pub strict_confirmation: bool,
    /// Desktop notification when erasures finish while the window is in the
    /// background. Off by default.
    #[serde(default)]
    pub notifications: bool,
}

fn settings_file(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

fn logo_file(dir: &Path, format: &str) -> PathBuf {
    dir.join(format!("logo.{format}"))
}

/// The settings in `dir`; defaults when absent or unreadable.
pub fn load(dir: &Path) -> Settings {
    std::fs::read(settings_file(dir))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn store(dir: &Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(settings_file(dir), json).map_err(|error| error.to_string())
}

/// A field as it will be signed: trimmed, empty means absent; `Err` when
/// too long or holding control characters.
fn field(text: Option<String>, max_chars: usize, name: &str) -> Result<Option<String>, String> {
    let Some(text) = text else {
        return Ok(None);
    };
    let text = text.trim();
    if text.chars().count() > max_chars {
        return Err(format!("{name}: {max_chars} characters at most"));
    }
    if text.chars().any(char::is_control) {
        return Err(format!("{name}: control characters are not allowed"));
    }
    Ok((!text.is_empty()).then(|| text.to_owned()))
}

/// Saves the organization and the operator.
pub fn save(
    dir: &Path,
    organization: Option<String>,
    operator: Option<String>,
) -> Result<Settings, String> {
    let mut settings = load(dir);
    settings.organization = field(organization, MAX_ORGANIZATION_CHARS, "organization")?;
    settings.operator = field(operator, MAX_OPERATOR_CHARS, "operator")?;
    store(dir, &settings)?;
    Ok(settings)
}

/// Turns the strict confirmation mode on or off.
pub fn set_strict_confirmation(dir: &Path, enabled: bool) -> Result<Settings, String> {
    let mut settings = load(dir);
    settings.strict_confirmation = enabled;
    store(dir, &settings)?;
    Ok(settings)
}

/// Turns the desktop notifications on or off.
pub fn set_notifications(dir: &Path, enabled: bool) -> Result<Settings, String> {
    let mut settings = load(dir);
    settings.notifications = enabled;
    store(dir, &settings)?;
    Ok(settings)
}

/// Format of a logo file: PNG by its signature, SVG by an `<svg` element
/// near its start. Anything else is refused.
pub fn logo_format(data: &[u8]) -> Result<&'static str, String> {
    if data.len() > MAX_LOGO_BYTES {
        return Err("logo: 1 MiB at most".to_owned());
    }
    if data.starts_with(PNG_SIGNATURE) {
        return Ok("png");
    }
    let head = &data[..data.len().min(4096)];
    if std::str::from_utf8(head).is_ok_and(|text| text.contains("<svg")) {
        return Ok("svg");
    }
    Err("logo: PNG or SVG expected".to_owned())
}

/// Stores the customer's logo, replacing the previous one.
pub fn set_logo(dir: &Path, data: &[u8]) -> Result<Settings, String> {
    let format = logo_format(data)?;
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    for other in ["png", "svg"] {
        let _ = std::fs::remove_file(logo_file(dir, other));
    }
    std::fs::write(logo_file(dir, format), data).map_err(|error| error.to_string())?;
    let mut settings = load(dir);
    settings.logo = Some(format.to_owned());
    store(dir, &settings)?;
    Ok(settings)
}

/// Removes the customer's logo: certificates show the disphas wordmark.
pub fn clear_logo(dir: &Path) -> Result<Settings, String> {
    for format in ["png", "svg"] {
        let _ = std::fs::remove_file(logo_file(dir, format));
    }
    let mut settings = load(dir);
    settings.logo = None;
    store(dir, &settings)?;
    Ok(settings)
}

/// The stored logo, if any, ready for rendering.
pub fn logo(dir: &Path) -> Option<(Vec<u8>, LogoFormat)> {
    let settings = load(dir);
    let (format, kind) = match settings.logo.as_deref()? {
        "png" => ("png", LogoFormat::Png),
        "svg" => ("svg", LogoFormat::Svg),
        _ => return None,
    };
    let data = std::fs::read(logo_file(dir, format)).ok()?;
    logo_format(&data).ok()?;
    Some((data, kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_trimmed_bounded_and_kept() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let saved = save(
            dir.path(),
            Some("  Atelier XYZ ".to_owned()),
            Some("".to_owned()),
        )
        .expect("saved");
        assert_eq!(saved.organization.as_deref(), Some("Atelier XYZ"));
        assert_eq!(saved.operator, None);
        assert_eq!(load(dir.path()), saved);
        assert!(save(dir.path(), Some("x".repeat(129)), None).is_err());
        assert!(save(dir.path(), None, Some("a\tb".to_owned())).is_err());
    }

    #[test]
    fn notifications_are_off_until_turned_on() {
        let dir = tempfile::tempdir().expect("temporary directory");
        // A settings file written before the option existed.
        std::fs::write(
            settings_file(dir.path()),
            br#"{"organization":null,"operator":null,"logo":null}"#,
        )
        .expect("written");
        assert!(!load(dir.path()).notifications);
        assert!(
            set_notifications(dir.path(), true)
                .expect("saved")
                .notifications
        );
        assert!(load(dir.path()).notifications);
    }

    #[test]
    fn logos_are_png_or_svg_only() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&[0; 16]);
        assert_eq!(
            set_logo(dir.path(), &png).expect("png").logo.as_deref(),
            Some("png")
        );
        assert!(matches!(logo(dir.path()), Some((_, LogoFormat::Png))));

        let svg = br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>"#;
        assert_eq!(
            set_logo(dir.path(), svg).expect("svg").logo.as_deref(),
            Some("svg")
        );
        assert!(
            !dir.path().join("logo.png").exists(),
            "previous logo replaced"
        );

        assert!(set_logo(dir.path(), b"GIF89a").is_err());
        assert!(set_logo(dir.path(), &vec![0u8; MAX_LOGO_BYTES + 1]).is_err());

        clear_logo(dir.path()).expect("cleared");
        assert!(logo(dir.path()).is_none());
    }
}
