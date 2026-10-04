//! Display helpers shared by every reader of an attestation: languages,
//! labels of the codes, dates and identifiers.
//!
//! Normative terms stay untranslated: Clear, Purge, NIST SP 800-88, IEEE 2883.
//! Codes without a label (from a newer Oblivio) are shown as is by callers.

/// Display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English.
    En,
    /// French.
    Fr,
    /// German.
    De,
}

impl Lang {
    /// Parses `en`, `fr` or `de`, or a locale such as `fr_FR.UTF-8`.
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag.get(..2)?.to_ascii_lowercase().as_str() {
            "en" => Some(Self::En),
            "fr" => Some(Self::Fr),
            "de" => Some(Self::De),
            _ => None,
        }
    }

    /// Language of the system (`LC_ALL`, `LC_MESSAGES`, `LANG`), English by default.
    pub fn from_env() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.is_empty())
            .and_then(|value| Self::from_tag(&value))
            .unwrap_or(Self::En)
    }
}

/// A text in the three languages.
#[derive(Debug, Clone, Copy)]
pub struct Text {
    en: &'static str,
    fr: &'static str,
    de: &'static str,
}

impl Text {
    /// A text given in English, French and German.
    pub const fn new(en: &'static str, fr: &'static str, de: &'static str) -> Self {
        Self { en, fr, de }
    }

    /// The text in `lang`.
    pub fn get(self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en,
            Lang::Fr => self.fr,
            Lang::De => self.de,
        }
    }
}

/// Every code this version has a label for, in display order.
pub const KNOWN_CODES: &[&str] = &[
    "N1",
    "N3",
    "N8",
    "N9",
    "N10",
    "D1",
    "D2",
    "D3",
    "D4",
    "D5",
    "R1",
    "R2",
    "R3",
    "R4",
    "R5",
    "R6",
    "R7",
    "R8",
    "NvmeSanitizeBlockErase",
    "NvmeSanitizeCryptoErase",
    "NvmeFormatCryptoErase",
    "NvmeFormatUserDataErase",
    "AtaSanitizeBlockErase",
    "AtaSanitizeCryptoScramble",
    "AtaSanitizeOverwrite",
    "AtaSecurityEraseEnhanced",
    "AtaSecurityEraseNormal",
    "HostOverwrite",
];

/// Label of a code found in an attestation, if this version knows it.
pub fn code(code: &str) -> Option<Text> {
    Some(match code {
        "N1" => Text::new(
            "Drive could not be inspected",
            "Inspection du disque impossible",
            "Laufwerk konnte nicht geprüft werden",
        ),
        "N3" => Text::new(
            "Media is read-only",
            "Support en lecture seule",
            "Medium ist schreibgeschützt",
        ),
        "N8" => Text::new(
            "Read errors during validation",
            "Erreurs de lecture pendant la validation",
            "Lesefehler bei der Validierung",
        ),
        "N9" => Text::new(
            "No executable method",
            "Aucune méthode exécutable",
            "Keine ausführbare Methode",
        ),
        "N10" => Text::new(
            "Verification or validation failed",
            "Vérification ou validation en échec",
            "Verifizierung oder Validierung fehlgeschlagen",
        ),
        "D1" => Text::new(
            "Firmware reports degraded reliability",
            "Le firmware signale une fiabilité dégradée",
            "Firmware meldet verminderte Zuverlässigkeit",
        ),
        "D2" => Text::new(
            "Volatile memory backup failed",
            "Sauvegarde de la mémoire volatile en échec",
            "Sicherung des flüchtigen Speichers fehlgeschlagen",
        ),
        "D3" => Text::new(
            "S.M.A.R.T. predicts a failure",
            "Le S.M.A.R.T. prédit une panne",
            "S.M.A.R.T. sagt einen Ausfall voraus",
        ),
        "D4" => Text::new(
            "High reallocated sector count",
            "Nombre élevé de secteurs réalloués",
            "Hohe Anzahl umgelagerter Sektoren",
        ),
        "D5" => Text::new(
            "Sectors reallocated during the erasure",
            "Secteurs réalloués pendant l'effacement",
            "Sektoren während der Löschung umgelagert",
        ),
        "R1" => Text::new(
            "Spare capacity below threshold",
            "Réserve de cellules sous le seuil",
            "Ersatzkapazität unter dem Schwellenwert",
        ),
        "R2" => Text::new(
            "Rated endurance exhausted",
            "Endurance nominale épuisée",
            "Nennlebensdauer erschöpft",
        ),
        "R3" => Text::new(
            "Media integrity errors recorded",
            "Erreurs d'intégrité du support enregistrées",
            "Integritätsfehler des Mediums aufgezeichnet",
        ),
        "R4" => Text::new(
            "Some sectors reallocated",
            "Quelques secteurs réalloués",
            "Einige Sektoren umgelagert",
        ),
        "R5" => Text::new(
            "Pending or uncorrectable sectors",
            "Secteurs en attente ou incorrigibles",
            "Ausstehende oder nicht korrigierbare Sektoren",
        ),
        "R6" => Text::new(
            "Uncorrectable errors reported",
            "Erreurs non corrigées signalées",
            "Nicht korrigierte Fehler gemeldet",
        ),
        "R7" => Text::new(
            "Host write: reallocated sectors not covered",
            "Écriture hôte : secteurs réalloués non couverts",
            "Hostschreibvorgang: umgelagerte Sektoren nicht abgedeckt",
        ),
        "R8" => Text::new(
            "Model listed in the Oblivio quirk base",
            "Modèle répertorié dans la base d'anomalies Oblivio",
            "Modell in der Oblivio-Anomaliedatenbank gelistet",
        ),
        "NvmeSanitizeBlockErase" => Text::new(
            "NVMe Sanitize, Block Erase",
            "NVMe Sanitize, Block Erase",
            "NVMe Sanitize, Block Erase",
        ),
        "NvmeSanitizeCryptoErase" => Text::new(
            "NVMe Sanitize, Crypto Erase",
            "NVMe Sanitize, Crypto Erase",
            "NVMe Sanitize, Crypto Erase",
        ),
        "NvmeFormatCryptoErase" => Text::new(
            "NVMe Format, cryptographic erase",
            "NVMe Format, effacement cryptographique",
            "NVMe Format, kryptografische Löschung",
        ),
        "NvmeFormatUserDataErase" => Text::new(
            "NVMe Format, user data erase",
            "NVMe Format, effacement des données",
            "NVMe Format, Löschung der Benutzerdaten",
        ),
        "AtaSanitizeBlockErase" => Text::new(
            "ATA Sanitize, Block Erase",
            "ATA Sanitize, Block Erase",
            "ATA Sanitize, Block Erase",
        ),
        "AtaSanitizeCryptoScramble" => Text::new(
            "ATA Sanitize, Crypto Scramble",
            "ATA Sanitize, Crypto Scramble",
            "ATA Sanitize, Crypto Scramble",
        ),
        "AtaSanitizeOverwrite" => Text::new(
            "ATA Sanitize, Overwrite",
            "ATA Sanitize, Overwrite",
            "ATA Sanitize, Overwrite",
        ),
        "AtaSecurityEraseEnhanced" => Text::new(
            "ATA Security Erase, enhanced",
            "ATA Security Erase, renforcé",
            "ATA Security Erase, erweitert",
        ),
        "AtaSecurityEraseNormal" => Text::new(
            "ATA Security Erase, normal",
            "ATA Security Erase, normal",
            "ATA Security Erase, normal",
        ),
        "HostOverwrite" => Text::new(
            "Host overwrite, one pass of a pseudo-random stream",
            "Réécriture hôte, une passe d'un flux pseudo-aléatoire",
            "Überschreiben durch den Host, ein Durchgang mit Zufallsdatenstrom",
        ),
        _ => return None,
    })
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

/// Calendar date and time (UTC) of an epoch timestamp:
/// `(year, month, day, hour, minute, second)`.
pub fn civil(epoch: u64) -> (u64, u64, u64, u64, u64, u64) {
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
    (
        year,
        month,
        day,
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60,
    )
}

/// UTC date and time of an epoch timestamp, `YYYY-MM-DD HH:MM:SS UTC`.
pub fn utc(epoch: u64) -> String {
    let (year, month, day, hour, minute, second) = civil(epoch);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_language_tags_and_locales() {
        assert_eq!(Lang::from_tag("fr"), Some(Lang::Fr));
        assert_eq!(Lang::from_tag("de_DE.UTF-8"), Some(Lang::De));
        assert_eq!(Lang::from_tag("EN"), Some(Lang::En));
        assert_eq!(Lang::from_tag("it_IT"), None);
        assert_eq!(Lang::from_tag("C"), None);
    }

    #[test]
    fn every_attestation_code_has_a_label() {
        let codes = [
            "N1", "N3", "N8", "N9", "N10", "D1", "D2", "D3", "D4", "D5", "R1", "R2", "R3", "R4",
            "R5", "R6", "R7", "R8",
        ];
        for code in codes {
            let label = super::code(code).unwrap_or_else(|| panic!("{code} has no label"));
            for lang in [Lang::En, Lang::Fr, Lang::De] {
                assert!(!label.get(lang).is_empty());
            }
        }
        assert!(super::code("Z99").is_none());
        for code in KNOWN_CODES {
            assert!(super::code(code).is_some(), "{code}");
        }
    }

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
}
