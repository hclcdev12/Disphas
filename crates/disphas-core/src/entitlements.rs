//! Entitlements: what each commercial offer allows (`docs/offres.md` §4).
//!
//! The helper derives the entitlements from the offer of the installed
//! license: no new data is signed or sent over the network. Like the
//! [`RuleSet`](crate::RuleSet), the table is explicit and versioned: changing
//! a value means publishing a new table version.

use semver::Version;

/// Commercial offer, as signed in a license (`off`).
///
/// Codes are frozen (`docs/offres.md` §2): they are signed in licenses and
/// printed on certificates, so they must never change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Offer {
    /// 30-day trial before purchase.
    Essai,
    /// 30-day pass for a one-off project.
    Passe,
    /// Freelancers and IT service companies.
    Pro,
    /// Small workshop or non-profit, 8 drives in parallel.
    Atelier8,
    /// Medium workshop, 16 drives in parallel.
    Atelier16,
    /// Large workshop or ITAD, 32 drives in parallel.
    Atelier32,
    /// Multi-site organisations, on quotation.
    Parc,
    /// Development root (`disphas-license::dev_root`). Recognised only in
    /// builds with debug assertions, like the development root itself.
    Dev,
}

impl Offer {
    /// Every offer, in the order of `docs/offres.md` §2, then `Dev`.
    pub const ALL: &'static [Self] = &[
        Self::Essai,
        Self::Passe,
        Self::Pro,
        Self::Atelier8,
        Self::Atelier16,
        Self::Atelier32,
        Self::Parc,
        Self::Dev,
    ];

    /// Code as signed in licenses.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Essai => "Essai",
            Self::Passe => "Passe",
            Self::Pro => "Pro",
            Self::Atelier8 => "Atelier8",
            Self::Atelier16 => "Atelier16",
            Self::Atelier32 => "Atelier32",
            Self::Parc => "Parc",
            Self::Dev => "Dev",
        }
    }

    /// Offer with exactly this code (case included), if any.
    ///
    /// `Dev` is only recognised in builds with debug assertions: in a release
    /// build, a license that says `Dev` is an unknown offer.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .filter(|offer| *offer != Self::Dev || cfg!(debug_assertions))
            .find(|offer| offer.code() == code)
    }
}

impl core::fmt::Display for Offer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.code())
    }
}

/// Maximum number of erasures running at the same time on the workstation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxParallel {
    /// At most this many erasures at once. Always at least 1 in the tables
    /// built here (checked by tests); 0 would refuse every erasure.
    Limited(u32),
    /// No limit other than one task per drive.
    Unlimited,
}

impl MaxParallel {
    /// Whether one more erasure may start while `running` are in progress.
    pub fn allows_another(self, running: usize) -> bool {
        match self {
            Self::Limited(max) => usize::try_from(max).map_or(true, |max| running < max),
            Self::Unlimited => true,
        }
    }
}

/// What an offer allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entitlements {
    /// Maximum number of erasures running at the same time.
    pub max_parallel: MaxParallel,
    /// Whether the customer's logo replaces the disphas wordmark on certificates.
    pub customer_logo: bool,
}

/// Versioned table `offer → entitlements`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementTable {
    /// Version of this table.
    pub version: Version,
    /// Entitlements of each offer. Every [`Offer`] appears exactly once.
    pub offers: Vec<(Offer, Entitlements)>,
    /// Offer whose entitlements apply to an unknown offer (`docs/offres.md`
    /// §4.3, rule 3): never a refusal to sign.
    pub fallback: Offer,
}

impl EntitlementTable {
    /// Table 1.0.0, matching `docs/offres.md` §2 and §4.3.
    pub fn baseline() -> Self {
        const fn limited(max: u32, customer_logo: bool) -> Entitlements {
            Entitlements {
                max_parallel: MaxParallel::Limited(max),
                customer_logo,
            }
        }
        const UNLIMITED: Entitlements = Entitlements {
            max_parallel: MaxParallel::Unlimited,
            customer_logo: true,
        };
        Self {
            version: Version::new(1, 0, 0),
            offers: vec![
                (Offer::Essai, limited(8, true)),
                (Offer::Passe, limited(8, true)),
                (Offer::Pro, limited(2, false)),
                (Offer::Atelier8, limited(8, true)),
                (Offer::Atelier16, limited(16, true)),
                (Offer::Atelier32, limited(32, true)),
                (Offer::Parc, UNLIMITED),
                (Offer::Dev, UNLIMITED),
            ],
            fallback: Offer::Pro,
        }
    }

    /// Entitlements of a known offer, falling back to [`Self::fallback`].
    fn of(&self, offer: Offer) -> Option<Entitlements> {
        self.offers
            .iter()
            .find(|(known, _)| *known == offer)
            .map(|(_, entitlements)| *entitlements)
    }

    /// Entitlements for the offer code of a license, compared exactly.
    ///
    /// An unknown code (manual v1 licenses, future offers, `Dev` in a release
    /// build) gets the entitlements of [`Self::fallback`]. If the table itself
    /// lacks the fallback, the most restrictive entitlements apply: one
    /// erasure at a time, no customer logo.
    pub fn for_offer(&self, code: &str) -> Entitlements {
        Offer::from_code(code)
            .and_then(|offer| self.of(offer))
            .or_else(|| self.of(self.fallback))
            .unwrap_or(Entitlements {
                max_parallel: MaxParallel::Limited(1),
                customer_logo: false,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limited(max: u32, customer_logo: bool) -> Entitlements {
        Entitlements {
            max_parallel: MaxParallel::Limited(max),
            customer_logo,
        }
    }

    #[test]
    fn codes_are_frozen() {
        let codes: Vec<&str> = Offer::ALL.iter().map(|offer| offer.code()).collect();
        assert_eq!(
            codes,
            [
                "Essai",
                "Passe",
                "Pro",
                "Atelier8",
                "Atelier16",
                "Atelier32",
                "Parc",
                "Dev"
            ]
        );
        assert_eq!(Offer::Atelier16.to_string(), "Atelier16");
    }

    #[test]
    fn every_offer_appears_once_with_a_usable_limit() {
        let table = EntitlementTable::baseline();
        for offer in Offer::ALL {
            let count = table.offers.iter().filter(|(o, _)| o == offer).count();
            assert_eq!(count, 1, "{offer}");
        }
        assert_eq!(table.offers.len(), Offer::ALL.len());
        for (offer, entitlements) in &table.offers {
            assert_ne!(
                entitlements.max_parallel,
                MaxParallel::Limited(0),
                "{offer}"
            );
        }
        assert_eq!(table.version, Version::new(1, 0, 0));
    }

    #[test]
    fn baseline_matches_the_offer_grid() {
        let table = EntitlementTable::baseline();
        assert_eq!(table.for_offer("Essai"), limited(8, true));
        assert_eq!(table.for_offer("Passe"), limited(8, true));
        assert_eq!(table.for_offer("Pro"), limited(2, false));
        assert_eq!(table.for_offer("Atelier8"), limited(8, true));
        assert_eq!(table.for_offer("Atelier16"), limited(16, true));
        assert_eq!(table.for_offer("Atelier32"), limited(32, true));
        let parc = table.for_offer("Parc");
        assert_eq!(parc.max_parallel, MaxParallel::Unlimited);
        assert!(parc.customer_logo);
    }

    #[test]
    fn unknown_offers_get_pro() {
        let table = EntitlementTable::baseline();
        let pro = table.for_offer("Pro");
        // Former offer names, other cases, padding, empty: exact comparison only.
        for code in [
            "Atelier",
            "pro",
            "PRO",
            " Pro",
            "Pro ",
            "",
            "Atelier 8",
            "Entreprise",
        ] {
            assert_eq!(table.for_offer(code), pro, "{code:?}");
        }
        assert_eq!(Offer::from_code("pro"), None);
    }

    #[test]
    fn missing_fallback_is_most_restrictive() {
        let mut table = EntitlementTable::baseline();
        table.offers.retain(|(offer, _)| *offer != Offer::Pro);
        assert_eq!(table.for_offer("Pro"), limited(1, false));
        assert_eq!(table.for_offer("Inconnue"), limited(1, false));
        assert_eq!(table.for_offer("Atelier8"), limited(8, true));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn dev_is_unlimited_in_debug_builds() {
        let table = EntitlementTable::baseline();
        assert_eq!(Offer::from_code("Dev"), Some(Offer::Dev));
        let dev = table.for_offer("Dev");
        assert_eq!(dev.max_parallel, MaxParallel::Unlimited);
        assert!(dev.customer_logo);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn dev_is_unknown_in_release_builds() {
        let table = EntitlementTable::baseline();
        assert_eq!(Offer::from_code("Dev"), None);
        assert_eq!(table.for_offer("Dev"), table.for_offer("Pro"));
    }

    #[test]
    fn parallel_limit() {
        let two = MaxParallel::Limited(2);
        assert!(two.allows_another(0));
        assert!(two.allows_another(1));
        assert!(!two.allows_another(2));
        assert!(!two.allows_another(3));
        assert!(!MaxParallel::Limited(0).allows_another(0));
        assert!(MaxParallel::Limited(u32::MAX).allows_another(1_000));
        assert!(MaxParallel::Unlimited.allows_another(usize::MAX));
        // Parc goes beyond the largest Atelier offer.
        let parc = EntitlementTable::baseline().for_offer("Parc");
        assert!(parc.max_parallel.allows_another(32));
    }
}
