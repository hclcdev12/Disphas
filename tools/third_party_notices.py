#!/usr/bin/env python3
"""Builds packaging/THIRD-PARTY-NOTICES.txt: the license notices of every
third-party component shipped in the disphas package (docs/conformite.md §5).

Sources, read as shipped, never typed by hand:
- Rust crates linked into disphas-helper and disphas-app on x86_64 Linux
  (`cargo metadata`, normal dependencies only), with the license files
  found in each crate package;
- npm packages bundled into the frontend (the list below is the one the
  Vite source map reports: re-check it when dependencies change);
- embedded fonts (assets/fonts).

Identical license texts are printed once, with the components they cover.

    python3 tools/third_party_notices.py          # write the file
    python3 tools/third_party_notices.py --check  # fail if it is stale
"""

import hashlib
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "packaging" / "THIRD-PARTY-NOTICES.txt"
SPDX_TEXTS = ROOT / "packaging" / "licenses"
TARGET = "x86_64-unknown-linux-gnu"
SHIPPED_BINARIES = ("disphas-helper", "disphas-app")
# From `vite build --sourcemap`: node_modules packages present in the bundle,
# plus tailwindcss whose generated CSS is shipped.
BUNDLED_NPM = (
    "@tauri-apps/api",
    "i18next",
    "lucide-react",
    "react",
    "react-dom",
    "react-i18next",
    "scheduler",
    "tailwindcss",
    "use-sync-external-store",
)
FONTS = (
    ("Inter", "assets/fonts/LICENSE-Inter.txt"),
    ("JetBrains Mono", "assets/fonts/LICENSE-JetBrainsMono.txt"),
)
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice|copyright|unlicense)([-._].*)?$", re.I)


def license_texts(directory: pathlib.Path) -> list[tuple[str, str]]:
    files = sorted(
        path
        for path in directory.iterdir()
        if path.is_file() and LICENSE_FILE.match(path.name)
    )
    return [(path.name, path.read_text(encoding="utf-8", errors="replace").strip()) for path in files]


def rust_components() -> list[dict]:
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET],
            cwd=ROOT,
        )
    )
    members = set(metadata["workspace_members"])
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    roots = [i for i in members if packages[i]["name"] in SHIPPED_BINARIES]
    if len(roots) != len(SHIPPED_BINARIES):
        sys.exit("shipped binaries not found in the workspace")
    reached, stack = set(), list(roots)
    while stack:
        current = stack.pop()
        if current in reached:
            continue
        reached.add(current)
        for dependency in nodes[current]["deps"]:
            if any(kind["kind"] is None for kind in dependency["dep_kinds"]):
                stack.append(dependency["pkg"])
    components = []
    for package_id in sorted(reached - members, key=lambda i: (packages[i]["name"], packages[i]["version"])):
        package = packages[package_id]
        directory = pathlib.Path(package["manifest_path"]).parent
        components.append(
            {
                "name": f"{package['name']} {package['version']} (Rust crate)",
                "license": package.get("license") or "see license file",
                "source": package.get("repository") or f"https://crates.io/crates/{package['name']}",
                "texts": license_texts(directory),
            }
        )
    return components


def npm_components() -> list[dict]:
    components = []
    for name in BUNDLED_NPM:
        directory = ROOT / "app" / "node_modules" / name
        manifest = json.loads((directory / "package.json").read_text(encoding="utf-8"))
        repository = manifest.get("repository")
        if isinstance(repository, dict):
            repository = repository.get("url")
        components.append(
            {
                "name": f"{name} {manifest['version']} (npm package)",
                "license": manifest.get("license") or "see license file",
                "source": repository or f"https://www.npmjs.com/package/{name}",
                "texts": license_texts(directory),
            }
        )
    return components


def font_components() -> list[dict]:
    return [
        {
            "name": f"{name} (font)",
            "license": "OFL-1.1",
            "source": "assets/fonts",
            "texts": [(pathlib.Path(path).name, (ROOT / path).read_text(encoding="utf-8").strip())],
        }
        for name, path in FONTS
    ]


def render(components: list[dict]) -> str:
    groups: dict[str, dict] = {}
    without_file = []
    for component in components:
        if not component["texts"]:
            without_file.append(component)
            continue
        body = "\n\n".join(f"--- {name} ---\n{text}" for name, text in component["texts"])
        key = hashlib.sha256(body.encode()).hexdigest()
        groups.setdefault(key, {"body": body, "components": []})["components"].append(component)

    lines = [
        "DISPHAS — THIRD-PARTY NOTICES",
        "",
        "disphas includes the third-party components listed below. Each is used",
        "under its own license, reproduced here as shipped by its authors.",
        "Generated by tools/third_party_notices.py; do not edit by hand.",
        "",
        f"Components: {len(components)}",
        "",
    ]
    for group in sorted(groups.values(), key=lambda g: g["components"][0]["name"].lower()):
        lines.append("=" * 78)
        for component in group["components"]:
            lines.append(f"{component['name']} — {component['license']} — {component['source']}")
        lines.append("")
        lines.append(group["body"])
        lines.append("")
    if without_file:
        lines.append("=" * 78)
        lines.append("Components whose package ships no license file. They are used under the")
        lines.append("license they declare; its text is available at the source given:")
        lines.append("")
        for component in without_file:
            lines.append(f"{component['name']} — {component['license']} — {component['source']}")
        lines.append("")
        identifiers = sorted(
            {
                token
                for component in without_file
                for token in re.split(r"[\s()/]+", component["license"])
                if token and token not in ("OR", "AND", "WITH")
            }
        )
        for identifier in identifiers:
            path = SPDX_TEXTS / f"{identifier}.txt"
            if not path.exists():
                sys.exit(f"no standard text for {identifier} in {SPDX_TEXTS.relative_to(ROOT)}")
            lines.append("=" * 78)
            lines.append(f"Standard text of {identifier} (SPDX License List)")
            lines.append("")
            lines.append(path.read_text(encoding="utf-8").strip())
            lines.append("")
    return "\n".join(lines)


def main() -> None:
    text = render(rust_components() + npm_components() + font_components())
    if "--check" in sys.argv[1:]:
        current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if current != text:
            sys.exit(f"{OUTPUT.relative_to(ROOT)} is stale: run tools/third_party_notices.py")
        print("third-party notices up to date")
        return
    OUTPUT.write_text(text, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
