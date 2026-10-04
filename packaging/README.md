# Packaging (Linux, V1)

`npm run tauri build` in `app/` builds the release helper, then the `.deb`:

| Installed path | Source | Role |
|---|---|---|
| `/usr/bin/oblivio-app` | `app/src-tauri` | Interface, runs as the user |
| `/usr/libexec/oblivio/oblivio-helper` | `crates/oblivio-helper` (release) | Root helper, launched by pkexec |
| `/usr/share/polkit-1/actions/org.oblivio.helper.policy` | `packaging/polkit/` | Administrator authentication (`auth_admin`, never kept, active local session only) |
| `/usr/share/doc/oblivio/THIRD-PARTY-NOTICES.txt` | `packaging/` (generated) | Licenses of third-party components |
| postinst | `packaging/debian/postinst` | Creates the `oblivio` group; restricts certificates of an earlier version |
| postrm | `packaging/debian/postrm` | `purge` deletes `/var/lib/oblivio` and the `oblivio` group |

**Uninstalling.** `sudo apt remove oblivio` removes the program and keeps the data
(workstation key, license, certificates). `sudo apt purge oblivio` deletes every data and
setting: `/var/lib/oblivio` and the `oblivio` group (Cyber Resilience Act, Annex I, Part I
(2)(m)). Certificates exported to `~/Documents/Oblivio` belong to the user and are not
touched. Files are deleted, not sanitized: on an SSD the workstation key may remain in
unmapped cells; erase the drive itself to destroy it for sure.

At runtime the helper keeps its state in `/var/lib/oblivio` (`keys/`, `license/` root-only;
`reports/` 0750/0640, group `oblivio`).

Operators who use the application must be in that group:
`sudo usermod -aG oblivio <user>`, then log out and in again.

A release helper refuses to start unless it runs as root and was started by pkexec.
Release builds contain neither the demo drives nor the development root key: until the
official root key exists, no license can be valid and no drive can be erased.

## Dependencies (checked 2026-10-04)

`Depends` lists every shared library the two binaries load, as computed by
`dpkg-shlibdeps` on the built binaries, plus the programs called at run time:
`pkexec` (helper launch), `xdg-utils` (`xdg-open`, opening certificates) and `passwd`
(`groupadd`/`groupdel` in the maintainer scripts). The app needs **glibc 2.39**:
Ubuntu 24.04 / Pop!_OS 24.04, Debian 13 or newer; the package refuses to install on
older systems instead of failing at launch. After a dependency or toolchain change:

```
dpkg-deb -x target/release/bundle/deb/*.deb /tmp/deb
mkdir -p /tmp/shlib/debian && printf 'Source: oblivio\n\nPackage: oblivio\nArchitecture: amd64\n' > /tmp/shlib/debian/control
cd /tmp/shlib && dpkg-shlibdeps -O /tmp/deb/usr/bin/oblivio-app /tmp/deb/usr/libexec/oblivio/oblivio-helper
```
