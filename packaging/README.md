# Packaging (Linux, V1)

`npm run tauri build` in `app/` builds the release helper, then the `.deb`:

| Installed path | Source | Role |
|---|---|---|
| `/usr/bin/oblivio-app` | `app/src-tauri` | Interface, runs as the user |
| `/usr/libexec/oblivio/oblivio-helper` | `crates/oblivio-helper` (release) | Root helper, launched by pkexec |
| `/usr/share/polkit-1/actions/org.oblivio.helper.policy` | `packaging/polkit/` | Administrator authentication (`auth_admin`, never kept, active local session only) |
| `/usr/share/doc/oblivio/THIRD-PARTY-NOTICES.txt` | `packaging/` (generated) | Licenses of third-party components |
| postinst | `packaging/debian/postinst` | Creates the root-only state tree; restricts certificates of an earlier version |
| postrm | `packaging/debian/postrm` | `purge` deletes `/var/lib/oblivio` |

**Uninstalling.** `sudo apt remove oblivio` removes the program and keeps the data
(workstation key, license, certificates). `sudo apt purge oblivio` deletes every data and
setting: `/var/lib/oblivio` (Cyber Resilience Act, Annex I, Part I
(2)(m)). Certificates exported to `~/Documents/Oblivio` belong to the user and are not
touched. Files are deleted, not sanitized: on an SSD the workstation key may remain in
unmapped cells; erase the drive itself to destroy it for sure.

At runtime the helper keeps its state in `/var/lib/oblivio` (`keys/`, `license/` root-only;
`reports/` root-only too: the application reads the certificates through the helper,
so no group and no account setting is needed).

A release helper refuses to start unless it runs as root and was started by pkexec.
Release builds contain neither the demo drives nor the development root key: until the
official root key exists, no license can be valid and no drive can be erased.

## Supported distributions and dependencies (2026-10-04)

Release packages are built by the **Package** workflow (`.github/workflows/package.yml`,
manual or on a `v*` tag) on **Ubuntu 22.04**, so the binaries need **glibc 2.35** at most:

| Package | Targets | Tested by installing on a clean container |
|---|---|---|
| `.deb` | Ubuntu 22.04+, Debian 12+, Linux Mint 21+, Pop!_OS 22.04+ | Ubuntu 22.04, 24.04, Debian 12, 13 |
| `.rpm` | Fedora, openSUSE (and RHEL-like with glibc 2.35+ and WebKitGTK 4.1) | Fedora (latest), openSUSE Tumbleweed |

Each run checks, per system: installation from a clean image, every library of both
programs resolved (`ldd`), polkit policy and notices in place, `/var/lib/oblivio` and its
sub-folders owned by root in `0700`, the release helper refusing to start outside pkexec,
and removal (`.deb`: `remove` keeps `/var/lib/oblivio`, `purge` deletes it; `.rpm`: removal
keeps the data).

An `oblivio` group left by a build earlier than commit `472cec3` (2026-10-06) is not removed, by upgrade or
purge: it no longer grants anything (certificates are root-only).

- `.deb` `Depends`: every library found by `dpkg-shlibdeps` (old and `t64` names as
  alternatives), plus `pkexec` and `xdg-utils` (opening certificates). `tools/check_package.py --max-glibc 2.35`
  fails the build if a library is undeclared or a program needs a newer glibc. A `.deb` built on a newer system
  (e.g. a developer's Pop!_OS 24.04) needs glibc 2.39 and must not be distributed.
- `.rpm` `Requires`: shared-library capabilities (`libwebkit2gtk-4.1.so.0()(64bit)`…) and
  file paths (`/usr/bin/pkexec`, `/usr/bin/xdg-open`…), so they resolve whatever each
  distribution names its packages. RPM has no purge: to delete every data and setting,
  `sudo rm -rf /var/lib/oblivio` (documented in `packaging/rpm/post`).
- AppImage is not offered: its root helper would live in a user-writable file, so an
  administrator could be led to run a tampered helper as root.
