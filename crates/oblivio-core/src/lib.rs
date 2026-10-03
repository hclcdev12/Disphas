//! Oblivio domain core: drive models, decision engine, versioned rule sets
//! and state machine.
//!
//! This crate performs no I/O and never touches a device: everything here is
//! pure and unit-testable. See `docs/moteur_decision.md`.

pub mod codes;
pub mod decide;
pub mod method;
pub mod profile;
pub mod rules;
pub mod verdict;

pub use decide::decide;
pub use rules::RuleSet;
pub use verdict::Verdict;
