#!/usr/bin/env python3
"""Checks a built disphas .deb before release (packaging/README.md):

- no shipped program needs a glibc newer than --max-glibc (2.35 covers
  Ubuntu 22.04, Debian 12 and newer);
- every library package `dpkg-shlibdeps` finds is declared in Depends
  (a `t64` renamed package and its old name count as the same).

    python3 tools/check_package.py target/release/bundle/deb/*.deb --max-glibc 2.35
"""

import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

PROGRAMS = ("usr/bin/disphas-app", "usr/libexec/disphas/disphas-helper")


def version(text: str) -> tuple[int, ...]:
    return tuple(int(part) for part in text.split("."))


def run(*args: str, cwd: Path | None = None) -> str:
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout


def base_name(package: str) -> str:
    return package.removesuffix("t64")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("deb", type=Path)
    parser.add_argument("--max-glibc", required=True)
    args = parser.parse_args()
    limit = version(args.max_glibc)
    failures = []

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "pkg"
        run("dpkg-deb", "-x", str(args.deb), str(root))
        programs = [root / program for program in PROGRAMS]
        for program in programs:
            if not program.is_file():
                failures.append(f"missing {program.relative_to(root)}")
                continue
            needed = re.findall(r"GLIBC_([0-9.]+)", run("objdump", "-T", str(program)))
            newest = max(needed, key=version, default="0")
            status = "ok" if version(newest) <= limit else "TOO NEW"
            print(f"{program.relative_to(root)}: needs glibc {newest} ({status})")
            if version(newest) > limit:
                failures.append(f"{program.relative_to(root)} needs glibc {newest} > {args.max_glibc}")

        work = Path(tmp) / "shlib"
        (work / "debian").mkdir(parents=True)
        (work / "debian" / "control").write_text(
            "Source: disphas\n\nPackage: disphas\nArchitecture: amd64\n"
        )
        output = run("dpkg-shlibdeps", "-O", *map(str, programs), cwd=work)
        found = output.split("=", 1)[1].strip()
        print(f"dpkg-shlibdeps: {found}")
        needed_packages = {
            base_name(item.strip().split()[0])
            for item in found.split(",")
            if item.strip()
        }

    declared = run("dpkg-deb", "-f", str(args.deb), "Depends")
    declared_packages = {
        base_name(alternative.strip().split()[0])
        for item in declared.split(",")
        for alternative in item.split("|")
        if alternative.strip()
    }
    for package in sorted(needed_packages - declared_packages):
        failures.append(f"{package} is needed but not declared in Depends")

    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    if not failures:
        print("package checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
