#!/bin/sh
# Screenshot of the test bench's display (:0) over SSH, as a PNG on this
# machine: lets an agent see the Oblivio interface it drives with xdotool.
# Needs the `oblivio-bench` SSH alias (docs/reprise.md) and python3; no
# root and nothing installed on the bench (xwd ships with it).
#
#   tools/bench/shot.sh [out.png] [scale]   (scale: keep 1 pixel in N, default 2)
set -eu
OUT=${1:-${TMPDIR:-/tmp}/oblivio-bench.png}
SCALE=${2:-2}
HERE=$(dirname "$0")
RAW=$(mktemp)
trap 'rm -f "$RAW"' EXIT
ssh -o BatchMode=yes oblivio-bench \
    'DISPLAY=:0 XAUTHORITY=$HOME/.Xauthority xwd -root -silent' > "$RAW"
python3 "$HERE/xwd2png.py" "$RAW" "$OUT" "$SCALE"
