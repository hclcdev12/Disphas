//! Oblivio root helper.
//!
//! Launched once per session via `pkexec`, it talks to the UI over a private
//! stdin/stdout JSON pipe. The decision engine, protections and signing run
//! here and only here. See `docs/protocole_ipc.md`.

fn main() {
    // Skeleton only: no protocol loop yet, and nothing touches a device.
}
