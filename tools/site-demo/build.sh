#!/bin/sh
# Builds the website's interactive demo (docs/site.md) into site/public/demo/app/:
# the application's interface on a simulated helper (app/src/demo/), and the
# fictional certificates rendered by the real renderer with test keys.
# Audits the result first: nothing is copied if it fails.
#
#   tools/site-demo/build.sh
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "== interface (demo build)"
(cd "$root/app" && npm run -s build:demo)
cp -R "$root/app/dist-demo" "$work/app"

echo "== fictional certificates"
mkdir -p "$work/app/pdf"
(cd "$root" && cargo run -q -p oblivio-report --features fixtures --example demo_certificates -- "$work/app/pdf")

echo "== audit"
"$root/tools/site-demo/audit.sh" "$work/app"

echo "== copy into site/public/demo/app"
rm -rf "$root/site/public/demo/app"
mkdir -p "$root/site/public/demo"
cp -R "$work/app" "$root/site/public/demo/app"
echo "done: $(du -sh "$root/site/public/demo/app" | cut -f1)"
