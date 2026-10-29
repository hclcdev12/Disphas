#!/bin/sh
# Builds the test packages (.deb and .rpm) of the last commit, the way the
# Package workflow does (.github/workflows/package.yml): Ubuntu 22.04, so the
# binaries need glibc 2.35 at most, then tools/check_package.py. For the test
# bench while GitHub Actions is unavailable (docs/reprise.md §4).
#
# Runs in a podman container from a clean copy of HEAD (`git archive`): local
# changes are left out. Nothing here touches a drive.
#
#   tools/bench/package.sh <work-dir>
#
# The packages land in <work-dir>/out, versioned `<version>+local.<commit>`.
# Cargo and npm caches stay in <work-dir> to speed up the next build.
set -eu

work=${1:?usage: tools/bench/package.sh <work-dir>}
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
commit=$(git -C "$root" rev-parse --short HEAD)
base=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -n 1)
version="$base+local.$commit"

mkdir -p "$work/cargo" "$work/npm" "$work/out"
rm -rf "$work/src"
mkdir "$work/src"
git -C "$root" archive HEAD | tar -x -C "$work/src"

# Node 24, as in .node-version; checked against the published SHA-256.
node=node-v24.22.2-linux-x64
node_sha=48cc0e21e048790e54c0cf5749c2725005313d652651c40b7a21feb9697aaf75

podman run --rm \
  -v "$work/src:/src:Z" -v "$work/cargo:/root/.cargo:Z" -v "$work/npm:/root/.npm:Z" -v "$work/out:/out:Z" \
  -e VERSION="$version" -e NODE="$node" -e NODE_SHA="$node_sha" \
  docker.io/library/ubuntu:22.04 sh -euc '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq curl xz-utils build-essential pkg-config file python3 \
      libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev >/dev/null
    curl -fsSLO "https://nodejs.org/dist/v24.22.2/$NODE.tar.xz"
    echo "$NODE_SHA  $NODE.tar.xz" | sha256sum -c -
    tar -xJf "$NODE.tar.xz" -C /opt
    export PATH="/opt/$NODE/bin:/root/.cargo/bin:$PATH"
    [ -x /root/.cargo/bin/rustup ] || curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
    cd /src/app
    npm ci --no-audit --no-fund
    npm run tauri build -- --bundles deb rpm --config "{\"version\": \"$VERSION\"}"
    cd /src
    python3 tools/check_package.py target/release/bundle/deb/*.deb --max-glibc 2.35
    cp target/release/bundle/deb/*.deb target/release/bundle/rpm/*.rpm /out/
  '
ls -l "$work/out"
