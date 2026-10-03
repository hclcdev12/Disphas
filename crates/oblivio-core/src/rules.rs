//! Versioned rule set: every threshold and policy choice of the engine.
//!
//! Nothing in the engine is a hard-coded constant: changing a value here means
//! publishing a new `version`, which is written into every certificate.

use semver::Version;

use crate::method::Category;

/// Parameters of the decision engine, identified by `rules_version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    /// `rules_version`, written into every certificate.
    pub version: Version,
    /// ATA attribute 5 value at which D4 applies; R4 applies below it.
    pub max_reallocated_sectors: u64,
    /// Failed unblock attempts after which a BLOCKED drive falls back (N9).
    pub max_unblock_attempts: u32,
    /// Retries allowed before verification or validation fails (N10).
    pub max_retries: u8,
    /// Random sample blocks read during validation.
    pub validation_samples: u32,
    /// Lowest accepted value for `validation_samples`.
    pub min_validation_samples: u32,
    /// Shortest accepted justification for forcing a SAY NO, in characters.
    pub min_force_justification_chars: usize,
    /// Categories of the methods still awaiting confirmation against IEEE 2883.
    pub categories: CategoryRules,
}

/// Certified category of methods whose classification is a policy choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryRules {
    /// NVMe Format NVM, SES = 2 (cryptographic erase).
    pub nvme_format_crypto_erase: Category,
    /// NVMe Format NVM, SES = 1 (user data erase).
    pub nvme_format_user_data_erase: Category,
    /// ATA Security Erase on a rotational drive.
    pub ata_security_erase_hdd: Category,
    /// ATA Security Erase on an SSD, unless the quirk base validates Purge.
    pub ata_security_erase_ssd: Category,
}

impl RuleSet {
    /// Rule set 1.0.0, matching `docs/moteur_decision.md`.
    pub fn baseline() -> Self {
        Self {
            version: Version::new(1, 0, 0),
            max_reallocated_sectors: 50,
            max_unblock_attempts: 3,
            max_retries: 1,
            validation_samples: 1000,
            min_validation_samples: 100,
            min_force_justification_chars: 20,
            categories: CategoryRules {
                nvme_format_crypto_erase: Category::Purge,
                nvme_format_user_data_erase: Category::Clear,
                ata_security_erase_hdd: Category::Purge,
                ata_security_erase_ssd: Category::Clear,
            },
        }
    }
}
