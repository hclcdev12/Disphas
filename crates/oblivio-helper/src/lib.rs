//! Oblivio root helper, as a library so that every piece is testable without
//! a process, root privileges or a real drive.
//!
//! The decision engine, protections and signing run here and only here.
//! See `docs/protocole_ipc.md`.

pub mod backend;
pub mod convert;
#[cfg(feature = "demo")]
pub mod demo;
pub mod serve;
pub mod session;
