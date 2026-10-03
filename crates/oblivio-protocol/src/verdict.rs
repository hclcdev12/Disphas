//! Wire form of the verdict sent with `Inspect` (`docs/moteur_decision.md` §6).

use serde::{Deserialize, Serialize};

use crate::codes::{
    Acknowledgement, BlockCode, Category, DispositionCode, Method, ProtectCode, RefusalCode,
    ReserveCode, ScopeWarning,
};

/// Decision for one drive, computed by the helper only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum Verdict {
    /// PROTECTED: the drive must not be touched.
    Protected {
        /// Every protection that applies.
        reasons: Vec<ProtectCode>,
    },
    /// BLOCKED: operator action needed first.
    Blocked {
        /// Every blocking condition.
        reasons: Vec<BlockCode>,
    },
    /// SAY NO: the erasure cannot be proven.
    SayNo {
        /// Every refusal reason.
        reasons: Vec<RefusalCode>,
        /// Method the operator may force, Clear at most, if any.
        forcible: Option<MethodChoice>,
    },
    /// READY: a compliant method is available.
    Ready {
        /// Preferred method.
        recommended: MethodChoice,
        /// Other allowed methods, in order of preference.
        alternatives: Vec<MethodChoice>,
        /// Recommended future of the drive.
        disposition: Disposition,
    },
}

/// An allowed method and what executing it implies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct MethodChoice {
    /// The method.
    pub method: Method,
    /// Category it would be certified under.
    pub category: Category,
    /// Reserves certified if this method is executed.
    pub reserves: Vec<ReserveCode>,
    /// The method erases every namespace of an NVMe controller.
    pub all_namespaces: bool,
}

impl MethodChoice {
    /// Every warning the operator must acknowledge to execute this method,
    /// sorted: disposition codes, then reserves, then scope warnings.
    pub fn required_acknowledgements(&self, disposition: &Disposition) -> Vec<Acknowledgement> {
        let mut acknowledgements: Vec<Acknowledgement> = match disposition {
            Disposition::Reuse {} => Vec::new(),
            Disposition::DestroyRecommended { reasons } => reasons
                .iter()
                .copied()
                .map(Acknowledgement::Disposition)
                .collect(),
        };
        acknowledgements.extend(self.reserves.iter().copied().map(Acknowledgement::Reserve));
        if self.all_namespaces {
            acknowledgements.push(Acknowledgement::Scope(ScopeWarning::AllNamespaces));
        }
        acknowledgements.sort();
        acknowledgements.dedup();
        acknowledgements
    }
}

/// Recommended future of the drive once its erasure is certified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Disposition {
    /// The drive can be reused.
    ///
    /// A struct variant on purpose: serde ignores unknown fields on unit
    /// variants of internally tagged enums, even with `deny_unknown_fields`.
    Reuse {},
    /// Physical destruction is recommended.
    DestroyRecommended {
        /// Every disposition reason.
        reasons: Vec<DispositionCode>,
    },
}
