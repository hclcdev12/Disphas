# Fuzzing

Coverage-guided fuzzing of every parser of untrusted input, with
[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (needs the nightly toolchain;
stable stays the default for the project).

| Target | Untrusted input |
|---|---|
| `protocol_request` | requests from the UI, as decoded by the root helper (+ round trip) |
| `protocol_server_message` | responses and events, as decoded by the app (+ round trip) |
| `helper_serve` | a whole helper session on arbitrary input — no drive, no license, no file written |
| `attestation` | signed attestations, QR text, CBOR payloads, revocation lists |
| `pdf_attachments` | certificate PDFs given to the app or to `oblivio-verify` |
| `license_text` | license texts, activation requests sent by customers |
| `device_decode` | IDENTIFY, logs and SMART data returned by drives |

```bash
cd fuzz
cargo +nightly fuzz run protocol_request corpus/protocol_request seeds/protocol_request -- -max_total_time=600
```

`seeds/` holds valid inputs to start from (committed). `corpus/` and `artifacts/` are
generated and ignored by git; a crash is written to `artifacts/<target>/` with the input
that triggers it, replayed with `cargo +nightly fuzz run <target> artifacts/<target>/<file>`.
