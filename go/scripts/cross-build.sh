#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
#
# Cross-compile the Go glpi-agent for the same target matrix the Rust release
# pipeline covers (.github/workflows/release.yml): Linux, Windows and macOS, each
# on x86_64 and aarch64.
#
# Because the optional IEC 61850 cgo module is off by default, every target is a
# pure-Go static build (CGO_ENABLED=0) and needs no cross C toolchain — the whole
# matrix builds from one host. That is the Phase 10 bake-off signal: contrast the
# Rust pipeline, which installs a per-triple toolchain for each target.
set -euo pipefail

cd "$(dirname "$0")/.."

# GOOS/GOARCH pairs mirroring the Rust target triples.
targets=(
  "linux/amd64"     # x86_64-unknown-linux-gnu
  "linux/arm64"     # aarch64-unknown-linux-gnu
  "windows/amd64"   # x86_64-pc-windows-msvc
  "windows/arm64"   # aarch64-pc-windows-msvc
  "darwin/amd64"    # x86_64-apple-darwin
  "darwin/arm64"    # aarch64-apple-darwin
)

outdir="${1:-dist}"
mkdir -p "$outdir"

for target in "${targets[@]}"; do
  goos="${target%/*}"
  goarch="${target#*/}"
  out="$outdir/glpi-agent-${goos}-${goarch}"
  [ "$goos" = "windows" ] && out="${out}.exe"

  echo "==> building $goos/$goarch -> $out"
  CGO_ENABLED=0 GOOS="$goos" GOARCH="$goarch" \
    go build -trimpath -ldflags "-s -w" -o "$out" ./cmd/glpi-agent
done

echo
echo "Artifacts in $outdir:"
ls -lh "$outdir"
