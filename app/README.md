# disphas-app

Tauri v2 shell and React interface. The interface decides nothing: every verdict
comes from the root helper (`docs/protocole_ipc.md`).

- `npm run dev` — browser preview on http://localhost:1420. Outside Tauri, the
  interface replays `src/dev/demo-recording.json`, answers recorded from
  `disphas-helper --features demo` (fictional drives). Erasure is unavailable.
- Demo helper: the locked drive `DEMO-LOCK-7A11` (B2) unlocks with the fictional
  user password `disphas`; five wrong attempts expire its counter (B6).
- `npm run demo` — builds the demo helper (fictional drives, image files) and
  starts the app on it. `npm run tauri dev` alone uses whatever helper was built last.
- `npm run build:demo` — the website's interactive demo: this interface on a
  simulated helper (`src/demo/api.ts`, fictional drives, accelerated erasures,
  pre-rendered fictional PDFs) into `dist-demo/`. Published only through
  `tools/site-demo/build.sh`, which audits it first (docs/site.md §9).
- `npm run bindings` — regenerates `src/bindings/` from the Rust protocol types (ts-rs).
- `src/dev/code-labels.json` — code labels for the preview, exported by
  `cargo run -p disphas-attestation --example labels_json`. The real app gets
  them from Rust at runtime.
