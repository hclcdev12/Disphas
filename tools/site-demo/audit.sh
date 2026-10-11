#!/bin/sh
# Audits a demo build before it is published (docs/site.md): it must hold the
# interface and fictional data only. Fails on source maps, local paths,
# calls to the real helper, keys, or files of an unexpected kind.
#
#   tools/site-demo/audit.sh <demo-dir>
set -eu

dir=${1:?usage: audit.sh <demo-dir>}
failed=0
fail() {
  echo "AUDIT FAILED: $1"
  failed=1
}

# Only what a static demo needs.
unexpected=$(find "$dir" -type f ! -name '*.html' ! -name '*.js' ! -name '*.css' ! -name '*.ttf' \
  ! -name '*.svg' ! -name '*.pdf')
[ -z "$unexpected" ] || fail "unexpected files: $unexpected"

# No source maps: the original code is never published.
find "$dir" -name '*.map' | grep -q . && fail "source map files"
grep -rl 'sourceMappingURL' "$dir" --include='*.js' --include='*.css' >/dev/null 2>&1 && fail "sourceMappingURL"

# Nothing from the build machine or the repository.
grep -rlE '/home/|/root/|/Users/|Documents/obl|src-tauri|crates/oblivio|\.rs:' "$dir" \
  --include='*.js' --include='*.css' --include='*.html' >/dev/null 2>&1 && fail "local paths or repository references"

# The demo never reaches the real helper.
grep -rlE "oblivio://event|[\"'\`](render_certificate|list_reports|verify_certificate|open_certificate)[\"'\`]|__TAURI_INTERNALS__" "$dir" \
  --include='*.js' >/dev/null 2>&1 && fail "calls to the real helper"

# No key material: an encoded license, issuer certificate or activation
# request (prefix then Base45), or a PEM key. A bare prefix, as in the
# license dialog's placeholder, is only text.
grep -rlE 'BEGIN [A-Z ]*PRIVATE KEY|(OBI1|OBL1|OBA1|OBR1):[0-9A-Z $%*+./:-]{20,}' "$dir" \
  --include='*.js' --include='*.html' >/dev/null 2>&1 && fail "keys, licenses or activation requests"

# Certificates: fictional demo drives only.
for pdf in "$dir"/pdf/*.pdf; do
  [ -e "$pdf" ] || continue
  case $(basename "$pdf") in
    DEMO-*) ;;
    *) fail "certificate not from the demo: $pdf" ;;
  esac
done

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "audit passed: $(find "$dir" -type f | wc -l) files, no source map, no local path, no helper call, no key"
