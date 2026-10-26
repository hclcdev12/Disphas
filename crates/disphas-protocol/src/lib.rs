//! Message types exchanged between the disphas UI and the root helper.
//!
//! This is disphas's security boundary: it is designed as if the UI could be
//! buggy or compromised. Every type crossing it is deserialized strictly
//! (`#[serde(deny_unknown_fields)]`). See `docs/protocole_ipc.md`.
//!
//! The crate does not depend on `disphas-core`: the UI never links the
//! decision engine, and the wire format evolves with [`PROTOCOL_VERSION`].

pub mod codes;
pub mod command;
pub mod event;
pub mod response;
pub mod secret;
pub mod verdict;
pub mod wire;

pub use command::Command;
pub use event::Event;
pub use response::ErrorCode;
pub use verdict::Verdict;
pub use wire::{MAX_MESSAGE_BYTES, Request, Response, ServerMessage};

/// Version of this protocol, exchanged in `Hello`.
///
/// 2 (2026-10-20): `LicenseStatusResult.entitlements`, `PARALLEL_LIMIT`.
pub const PROTOCOL_VERSION: u32 = 3;
