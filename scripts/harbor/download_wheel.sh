#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/wheels" && pwd)"
mkdir -p "$DIR"
cd "$DIR"
VERSION="${1:-8.2.0}"
WHEEL="a3s_code-${VERSION}-cp310-abi3-manylinux_2_28_x86_64.whl"
URL="https://github.com/A3S-Lab/Code/releases/download/v${VERSION}/${WHEEL}"
echo "fetching ${URL}"
if [[ -f "$WHEEL" ]]; then
  echo "already present: $WHEEL"
  ls -lh "$WHEEL"
  exit 0
fi
curl -L --fail --retry 5 --retry-delay 2 -o "${WHEEL}.partial" "$URL"
mv "${WHEEL}.partial" "$WHEEL"
ls -lh "$WHEEL"
