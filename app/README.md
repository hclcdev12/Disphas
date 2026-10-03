# oblivio-app

Tauri v2 shell and React interface. The interface decides nothing: every verdict
comes from the root helper (`docs/protocole_ipc.md`).

- `npm run dev` — browser preview on http://localhost:1420. Outside Tauri, the
  interface replays `src/dev/demo-recording.json`, answers recorded from
  `oblivio-helper --features demo` (fictional drives). Erasure is unavailable.
- `npm run bindings` — regenerates `src/bindings/` from the Rust protocol types (ts-rs).
- `src/dev/code-labels.json` — code labels for the preview, exported by
  `cargo run -p oblivio-attestation --example labels_json`. The real app gets
  them from Rust at runtime.
