# Test fixtures

- `certificate.pdf`: a COMPLIANT certificate rendered by disphas from **fictional** data
  (drive `FICT0000A001`, "Atelier Œuvre & Łódź") and **test keys** only: root key and
  workstation key are the fixed seeds `[1; 32]` and `[2; 32]`, public and worthless.
  It carries `attestation.cose` and `journal.json` as attachments, like a real certificate.
- `batch.pdf`: a batch certificate (batch `LOT-2026-001`) of six fictional drives
  (`FICT0000A001` to `FICT0000A006`), same test keys, each attestation and journal attached.
