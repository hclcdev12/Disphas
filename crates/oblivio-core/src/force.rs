//! Forcing an erasure on a SAY NO drive (`docs/cadre_projet.md` §5.3).
//!
//! A forced erasure is capped at Clear and can never be certified compliant:
//! at best it yields a NON CONFORME certificate stating that physical
//! destruction is required, with the operator's justification.

use thiserror::Error;

use crate::codes::RefusalCode;
use crate::method::MethodOption;
use crate::rules::RuleSet;
use crate::verdict::Verdict;

/// An accepted request to force a SAY NO verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Forced {
    /// Method to execute, certified Clear at most.
    pub option: MethodOption,
    /// Refusal reasons of the verdict being forced.
    pub refusals: Vec<RefusalCode>,
    /// Operator justification, kept exactly as typed.
    pub justification: String,
}

/// Why a force request is rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ForceError {
    /// Only a SAY NO verdict can be forced.
    #[error("only a SAY NO verdict can be forced")]
    NotSayNo,
    /// The refusal reasons do not allow forcing.
    #[error("refusal {0:?} cannot be forced")]
    NotForcible(Vec<RefusalCode>),
    /// The justification is shorter than the rule set requires.
    #[error("justification has {actual} characters, at least {required} required")]
    JustificationTooShort {
        /// Characters required by the rule set.
        required: usize,
        /// Characters provided, surrounding whitespace excluded.
        actual: usize,
    },
}

/// Accepts or rejects forcing `verdict` with `justification`.
pub fn force(
    verdict: &Verdict,
    justification: &str,
    rules: &RuleSet,
) -> Result<Forced, ForceError> {
    let Verdict::SayNo { reasons, forcible } = verdict else {
        return Err(ForceError::NotSayNo);
    };
    let option = forcible.ok_or_else(|| ForceError::NotForcible(reasons.clone()))?;
    let actual = justification.trim().chars().count();
    if actual < rules.min_force_justification_chars {
        return Err(ForceError::JustificationTooShort {
            required: rules.min_force_justification_chars,
            actual,
        });
    }
    Ok(Forced {
        option,
        refusals: reasons.clone(),
        justification: justification.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::method::{Category, Method};

    const JUSTIFICATION: &str = "Disque client, IDENTIFY muet, écriture tentée";

    fn rules() -> RuleSet {
        RuleSet::baseline()
    }

    #[test]
    fn n1_is_forcible_with_a_host_write_capped_at_clear() {
        let verdict = Verdict::say_no(vec![RefusalCode::InspectionFailed]);
        let forced = force(&verdict, JUSTIFICATION, &rules()).expect("N1 is forcible");
        assert_eq!(
            forced.option,
            MethodOption {
                method: Method::HostOverwrite,
                category: Category::Clear
            }
        );
        assert_eq!(forced.refusals, [RefusalCode::InspectionFailed]);
        assert_eq!(forced.justification, JUSTIFICATION);
    }

    #[test]
    fn n3_and_n9_are_not_forcible() {
        for code in [RefusalCode::ReadOnlyMedia, RefusalCode::NoMethodAvailable] {
            let verdict = Verdict::say_no(vec![code]);
            assert_eq!(
                force(&verdict, JUSTIFICATION, &rules()),
                Err(ForceError::NotForcible(vec![code]))
            );
        }
    }

    #[test]
    fn only_say_no_can_be_forced() {
        let verdict = Verdict::Protected {
            reasons: vec![crate::codes::ProtectCode::MountedPartition],
        };
        assert_eq!(
            force(&verdict, JUSTIFICATION, &rules()),
            Err(ForceError::NotSayNo)
        );
    }

    #[test]
    fn justification_length_comes_from_the_rule_set_and_counts_characters() {
        let verdict = Verdict::say_no(vec![RefusalCode::InspectionFailed]);
        // 19 characters, several of them multi-byte, padded with whitespace.
        let short = "  Écriture forcée ici  ";
        assert_eq!(
            force(&verdict, short, &rules()),
            Err(ForceError::JustificationTooShort {
                required: 20,
                actual: 19
            })
        );
        let lenient = RuleSet {
            min_force_justification_chars: 19,
            ..rules()
        };
        assert!(force(&verdict, short, &lenient).is_ok());
    }
}
