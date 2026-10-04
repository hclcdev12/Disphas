//! Oblivio root helper, as a library so that every piece is testable without
//! a process, root privileges or a real drive.
//!
//! The decision engine, protections and signing run here and only here.
//! See `docs/protocole_ipc.md`.

pub mod backend;
pub mod convert;
#[cfg(feature = "demo")]
pub mod demo;
pub mod erasure;
pub mod job;
pub mod licensing;
pub mod serve;
pub mod session;
pub mod startup;
#[cfg(any(not(debug_assertions), feature = "real-devices"))]
pub mod system;

#[cfg(test)]
mod job_tests;
