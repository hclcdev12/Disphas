//! Message types exchanged between the Oblivio UI and the root helper.
//!
//! Every type crossing this boundary is deserialized strictly
//! (`#[serde(deny_unknown_fields)]`). See `docs/protocole_ipc.md`.
