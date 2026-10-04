# Packaging (Linux, V1)

`npm run tauri build` in `app/` builds the release helper, then the `.deb`:

| Installed path | Source | Role |
|---|---|---|
| `/usr/bin/oblivio-app` | `app/src-tauri` | Interface, runs as the user |
| `/usr/libexec/oblivio/oblivio-helper` | `crates/oblivio-helper` (release) | Root helper, launched by pkexec |
| `/usr/share/polkit-1/actions/org.oblivio.helper.policy` | `packaging/polkit/` | Administrator authentication (`auth_admin`, never kept, active local session only) |
| `/usr/share/doc/oblivio/THIRD-PARTY-NOTICES.txt` | `packaging/` (generated) | Licenses of third-party components |
| postinst | `packaging/debian/postinst` | Creates the `oblivio` group; restricts certificates of an earlier version |

At runtime the helper keeps its state in `/var/lib/oblivio` (`keys/`, `license/` root-only;
`reports/` 0750/0640, group `oblivio`).

Operators who use the application must be in that group:
`sudo usermod -aG oblivio <user>`, then log out and in again.

A release helper refuses to start unless it runs as root and was started by pkexec.
Release builds contain neither the demo drives nor the development root key: until the
official root key exists, no license can be valid and no drive can be erased.
