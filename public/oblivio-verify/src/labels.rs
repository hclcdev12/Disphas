//! Labels in English, French and German.
//!
//! Normative terms stay untranslated: Clear, Purge, NIST SP 800-88, IEEE 2883.
//! Codes without a label (from a newer Oblivio) are shown as is.

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
    /// The text in `lang`.
    pub fn get(self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en,
            Lang::Fr => self.fr,
            Lang::De => self.de,
        }
    }
}

const fn t(en: &'static str, fr: &'static str, de: &'static str) -> Text {
    Text { en, fr, de }
}

/// Label of a code found in an attestation, if this version knows it.
pub fn code(code: &str) -> Option<Text> {
    Some(match code {
        "N1" => t(
            "Drive could not be inspected",
            "Inspection du disque impossible",
            "Laufwerk konnte nicht geprüft werden",
        ),
        "N3" => t(
            "Media is read-only",
            "Support en lecture seule",
            "Medium ist schreibgeschützt",
        ),
        "N8" => t(
            "Read errors during validation",
            "Erreurs de lecture pendant la validation",
            "Lesefehler bei der Validierung",
        ),
        "N9" => t(
            "No executable method",
            "Aucune méthode exécutable",
            "Keine ausführbare Methode",
        ),
        "N10" => t(
            "Verification or validation failed",
            "Vérification ou validation en échec",
            "Verifizierung oder Validierung fehlgeschlagen",
        ),
        "D1" => t(
            "Firmware reports degraded reliability",
            "Le firmware signale une fiabilité dégradée",
            "Firmware meldet verminderte Zuverlässigkeit",
        ),
        "D2" => t(
            "Volatile memory backup failed",
            "Sauvegarde de la mémoire volatile en échec",
            "Sicherung des flüchtigen Speichers fehlgeschlagen",
        ),
        "D3" => t(
            "S.M.A.R.T. predicts a failure",
            "Le S.M.A.R.T. prédit une panne",
            "S.M.A.R.T. sagt einen Ausfall voraus",
        ),
        "D4" => t(
            "High reallocated sector count",
            "Nombre élevé de secteurs réalloués",
            "Hohe Anzahl umgelagerter Sektoren",
        ),
        "D5" => t(
            "Sectors reallocated during the erasure",
            "Secteurs réalloués pendant l'effacement",
            "Sektoren während der Löschung umgelagert",
        ),
        "R1" => t(
            "Spare capacity below threshold",
            "Réserve de cellules sous le seuil",
            "Ersatzkapazität unter dem Schwellenwert",
        ),
        "R2" => t(
            "Rated endurance exhausted",
            "Endurance nominale épuisée",
            "Nennlebensdauer erschöpft",
        ),
        "R3" => t(
            "Media integrity errors recorded",
            "Erreurs d'intégrité du support enregistrées",
            "Integritätsfehler des Mediums aufgezeichnet",
        ),
        "R4" => t(
            "Some sectors reallocated",
            "Quelques secteurs réalloués",
            "Einige Sektoren umgelagert",
        ),
        "R5" => t(
            "Pending or uncorrectable sectors",
            "Secteurs en attente ou incorrigibles",
            "Ausstehende oder nicht korrigierbare Sektoren",
        ),
        "R6" => t(
            "Uncorrectable errors reported",
            "Erreurs non corrigées signalées",
            "Nicht korrigierte Fehler gemeldet",
        ),
        "R7" => t(
            "Host write: reallocated sectors not covered",
            "Écriture hôte : secteurs réalloués non couverts",
            "Hostschreibvorgang: umgelagerte Sektoren nicht abgedeckt",
        ),
        "R8" => t(
            "Model listed in the Oblivio quirk base",
            "Modèle répertorié dans la base d'anomalies Oblivio",
            "Modell in der Oblivio-Anomaliedatenbank gelistet",
        ),
        "NvmeSanitizeBlockErase" => t(
            "NVMe Sanitize, Block Erase",
            "NVMe Sanitize, Block Erase",
            "NVMe Sanitize, Block Erase",
        ),
        "NvmeSanitizeCryptoErase" => t(
            "NVMe Sanitize, Crypto Erase",
            "NVMe Sanitize, Crypto Erase",
            "NVMe Sanitize, Crypto Erase",
        ),
        "NvmeFormatCryptoErase" => t(
            "NVMe Format, cryptographic erase",
            "NVMe Format, effacement cryptographique",
            "NVMe Format, kryptografische Löschung",
        ),
        "NvmeFormatUserDataErase" => t(
            "NVMe Format, user data erase",
            "NVMe Format, effacement des données",
            "NVMe Format, Löschung der Benutzerdaten",
        ),
        "AtaSanitizeBlockErase" => t(
            "ATA Sanitize, Block Erase",
            "ATA Sanitize, Block Erase",
            "ATA Sanitize, Block Erase",
        ),
        "AtaSanitizeCryptoScramble" => t(
            "ATA Sanitize, Crypto Scramble",
            "ATA Sanitize, Crypto Scramble",
            "ATA Sanitize, Crypto Scramble",
        ),
        "AtaSanitizeOverwrite" => t(
            "ATA Sanitize, Overwrite",
            "ATA Sanitize, Overwrite",
            "ATA Sanitize, Overwrite",
        ),
        "AtaSecurityEraseEnhanced" => t(
            "ATA Security Erase, enhanced",
            "ATA Security Erase, renforcé",
            "ATA Security Erase, erweitert",
        ),
        "AtaSecurityEraseNormal" => t(
            "ATA Security Erase, normal",
            "ATA Security Erase, normal",
            "ATA Security Erase, normal",
        ),
        "HostOverwrite" => t(
            "Host overwrite, one pass of zeros",
            "Réécriture hôte, une passe de zéros",
            "Überschreiben durch den Host, ein Durchgang mit Nullen",
        ),
        _ => return None,
    })
}

/// Interface messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Msg {
    Authentic,
    NotAuthentic,
    UnofficialRoot,
    Status,
    Compliant,
    NonCompliant,
    Refused,
    DestructionRequired,
    Report,
    Date,
    Drive,
    Model,
    Serial,
    Firmware,
    Capacity,
    Bytes,
    Identifiers,
    Sanitization,
    Category,
    Method,
    Standards,
    Verification,
    Validation,
    Canaries,
    Pattern,
    Samples,
    Passed,
    Failed,
    NotRun,
    Future,
    Reuse,
    DestroyRecommended,
    Reserves,
    Refusals,
    None,
    Justification,
    Organization,
    Operator,
    Tool,
    License,
    Journal,
    JournalMatches,
    JournalMismatch,
    NoLabel,
    ErrNotOblivio,
    ErrCorrupted,
    ErrMalformed,
    ErrLicense,
    ErrLicenseNotValid,
    ErrLicenseRevoked,
    ErrSignature,
}

/// Text of an interface message.
pub fn msg(message: Msg) -> Text {
    match message {
        Msg::Authentic => t(
            "AUTHENTIC: signed by a licensed Oblivio workstation",
            "AUTHENTIQUE : signée par un poste Oblivio sous licence",
            "ECHT: von einem lizenzierten Oblivio-Arbeitsplatz signiert",
        ),
        Msg::NotAuthentic => t("NOT AUTHENTIC", "NON AUTHENTIQUE", "NICHT ECHT"),
        Msg::UnofficialRoot => t(
            "Warning: verified against a root key given on the command line, not the official Oblivio key",
            "Attention : vérifié avec une clé racine fournie en paramètre, pas la clé officielle Oblivio",
            "Warnung: mit einem per Parameter übergebenen Stammschlüssel geprüft, nicht mit dem offiziellen Oblivio-Schlüssel",
        ),
        Msg::Status => t("Status", "Statut", "Status"),
        Msg::Compliant => t("COMPLIANT", "CONFORME", "KONFORM"),
        Msg::NonCompliant => t("NON-COMPLIANT", "NON CONFORME", "NICHT KONFORM"),
        Msg::Refused => t("REFUSED", "REFUS", "VERWEIGERT"),
        Msg::DestructionRequired => t(
            "Physical destruction required",
            "Destruction physique requise",
            "Physische Vernichtung erforderlich",
        ),
        Msg::Report => t("Report", "Rapport", "Bericht"),
        Msg::Date => t("Date", "Date", "Datum"),
        Msg::Drive => t("Drive", "Support", "Laufwerk"),
        Msg::Model => t("Model", "Modèle", "Modell"),
        Msg::Serial => t("Serial number", "Numéro de série", "Seriennummer"),
        Msg::Firmware => t("Firmware", "Firmware", "Firmware"),
        Msg::Capacity => t("Capacity", "Capacité", "Kapazität"),
        Msg::Bytes => t("bytes", "octets", "Bytes"),
        Msg::Identifiers => t("Identifiers", "Identifiants", "Kennungen"),
        Msg::Sanitization => t("Sanitization", "Sanitisation", "Bereinigung"),
        Msg::Category => t("Category", "Catégorie", "Kategorie"),
        Msg::Method => t("Method", "Méthode", "Methode"),
        Msg::Standards => t("Standards", "Référentiels", "Normen"),
        Msg::Verification => t("Verification", "Vérification", "Verifizierung"),
        Msg::Validation => t("Validation", "Validation", "Validierung"),
        Msg::Canaries => t("canaries", "canaris", "Kanarien"),
        Msg::Pattern => t("pattern", "motif", "Muster"),
        Msg::Samples => t("samples", "échantillons", "Stichproben"),
        Msg::Passed => t("passed", "réussie", "bestanden"),
        Msg::Failed => t("failed", "en échec", "fehlgeschlagen"),
        Msg::NotRun => t("not run", "non exécutée", "nicht ausgeführt"),
        Msg::Future => t(
            "Recommended future",
            "Devenir recommandé",
            "Empfohlene Verwendung",
        ),
        Msg::Reuse => t("Reuse", "Réemploi", "Wiederverwendung"),
        Msg::DestroyRecommended => t(
            "Physical destruction recommended",
            "Destruction physique recommandée",
            "Physische Vernichtung empfohlen",
        ),
        Msg::Reserves => t("Reserves", "Réserves", "Vorbehalte"),
        Msg::Refusals => t(
            "Refusal reasons",
            "Raisons du refus",
            "Gründe der Verweigerung",
        ),
        Msg::None => t("none", "aucune", "keine"),
        Msg::Justification => t(
            "Forcing justification",
            "Justification du forçage",
            "Begründung des Erzwingens",
        ),
        Msg::Organization => t("Organization", "Organisation", "Organisation"),
        Msg::Operator => t("Operator", "Opérateur", "Bediener"),
        Msg::Tool => t("Tool", "Outil", "Werkzeug"),
        Msg::License => t("License", "Licence", "Lizenz"),
        Msg::Journal => t("Detailed journal", "Journal détaillé", "Detailprotokoll"),
        Msg::JournalMatches => t(
            "matches the attestation",
            "correspond à l'attestation",
            "stimmt mit der Bescheinigung überein",
        ),
        Msg::JournalMismatch => t(
            "DOES NOT match the attestation",
            "NE correspond PAS à l'attestation",
            "stimmt NICHT mit der Bescheinigung überein",
        ),
        Msg::NoLabel => t(
            "no label in this version",
            "pas de libellé dans cette version",
            "keine Bezeichnung in dieser Version",
        ),
        Msg::ErrNotOblivio => t(
            "this is not an Oblivio certificate",
            "ce n'est pas un certificat Oblivio",
            "dies ist kein Oblivio-Zertifikat",
        ),
        Msg::ErrCorrupted => t(
            "the content is corrupted",
            "le contenu est corrompu",
            "der Inhalt ist beschädigt",
        ),
        Msg::ErrMalformed => t(
            "the attestation is malformed or was altered",
            "l'attestation est mal formée ou a été altérée",
            "die Bescheinigung ist fehlerhaft oder wurde verändert",
        ),
        Msg::ErrLicense => t(
            "the workstation license is not signed by Oblivio",
            "la licence du poste n'est pas signée par Oblivio",
            "die Arbeitsplatzlizenz ist nicht von Oblivio signiert",
        ),
        Msg::ErrLicenseNotValid => t(
            "the license was not valid on the attestation date",
            "la licence n'était pas valide à la date de l'attestation",
            "die Lizenz war zum Datum der Bescheinigung nicht gültig",
        ),
        Msg::ErrLicenseRevoked => t(
            "the license was revoked before the attestation date",
            "la licence a été révoquée avant la date de l'attestation",
            "die Lizenz wurde vor dem Datum der Bescheinigung widerrufen",
        ),
        Msg::ErrSignature => t(
            "the attestation is not signed by the licensed workstation",
            "l'attestation n'est pas signée par le poste sous licence",
            "die Bescheinigung ist nicht vom lizenzierten Arbeitsplatz signiert",
        ),
    }
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
    }
}
