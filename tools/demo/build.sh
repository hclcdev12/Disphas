#!/bin/sh
# Builds the interactive demo (docs/site.md §9) into demo/ at the root of the
# repository: the application's interface on a simulated helper
# (app/src/demo/), and the fictional certificates rendered by the real renderer
# with test keys. A standalone static folder, served from any address (relative
# paths). Audits the result first: nothing is copied if it fails.
#
#   tools/demo/build.sh
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "== interface (demo build)"
(cd "$root/app" && npm run -s build:demo)
cp -R "$root/app/dist-demo" "$work/app"

echo "== fictional certificates"
mkdir -p "$work/app/pdf"
(cd "$root" && cargo run -q -p disphas-report --features fixtures --example demo_certificates -- "$work/app/pdf")

echo "== audit"
"$root/tools/demo/audit.sh" "$work/app"

echo "== copy into demo/"
rm -rf "$root/demo"
cp -R "$work/app" "$root/demo"
echo "done: $(du -sh "$root/demo" | cut -f1)"
