//! disphas domain core: drive models, decision engine, versioned rule sets,
//! entitlements per offer and state machine.
//!
//! This crate performs no I/O and never touches a device: everything here is
//! pure and unit-testable. See `docs/moteur_decision.md`.

pub mod codes;
pub mod decide;
pub mod entitlements;
pub mod force;
pub mod method;
pub mod outcome;
pub mod profile;
pub mod rules;
pub mod verdict;

pub use decide::decide;
pub use entitlements::{EntitlementTable, Entitlements, MaxParallel, Offer};
pub use force::force;
pub use outcome::{Evaluation, evaluate_attempt, evaluate_forced_attempt};
pub use rules::RuleSet;
pub use verdict::Verdict;
