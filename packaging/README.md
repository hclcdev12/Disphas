# Packaging (Linux, V1)

`npm run tauri build` in `app/` builds the release helper, then the `.deb`:

| Installed path | Source | Role |
|---|---|---|
| `/usr/bin/oblivio-app` | `app/src-tauri` | Interface, runs as the user |
| `/usr/libexec/oblivio/oblivio-helper` | `crates/oblivio-helper` (release) | Root helper, launched by pkexec |
| `/usr/share/polkit-1/actions/org.oblivio.helper.policy` | `packaging/polkit/` | Administrator authentication (`auth_admin`, never kept, active local session only) |

At runtime the helper keeps its state in `/var/lib/oblivio` (`keys/`, `license/` root-only;
`reports/` readable by users).

A release helper refuses to start unless it runs as root and was started by pkexec.
Release builds contain neither the demo drives nor the development root key: until the
official root key exists, no license can be valid and no drive can be erased.
