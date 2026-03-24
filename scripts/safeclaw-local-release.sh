#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="${SAFECLAW_APP_DIR:-$ROOT_DIR/apps/safeclaw}"
REPO="${SAFECLAW_RELEASE_REPO:-A3S-Lab/SafeClaw}"
KEY_PATH="${SAFECLAW_TAURI_KEY_PATH:-$HOME/.tauri/safeclaw-updater.key}"
PUBKEY_PATH="${SAFECLAW_TAURI_PUBKEY_PATH:-${KEY_PATH}.pub}"
PASSWORD_FILE_DEFAULT="${SAFECLAW_TAURI_KEY_PASSWORD_FILE:-$HOME/.tauri/safeclaw-updater.key.password}"
PASSWORD="${SAFECLAW_TAURI_KEY_PASSWORD:-}"
TARGET="${SAFECLAW_BUILD_TARGET:-}"
CREATE_RELEASE="${SAFECLAW_CREATE_RELEASE:-1}"
BUNDLES="${SAFECLAW_BUNDLES:-app}"

if [[ -z "$PASSWORD" && -f "$PASSWORD_FILE_DEFAULT" ]]; then
  PASSWORD="$(cat "$PASSWORD_FILE_DEFAULT")"
fi

for cmd in gh pnpm node python3; do
  command -v "$cmd" >/dev/null 2>&1 || {
    echo "missing required command: $cmd" >&2
    exit 1
  }
done

[[ -d "$APP_DIR" ]] || {
  echo "SafeClaw app directory not found: $APP_DIR" >&2
  exit 1
}

[[ -f "$KEY_PATH" ]] || {
  echo "Updater private key not found: $KEY_PATH" >&2
  exit 1
}

[[ -f "$PUBKEY_PATH" ]] || {
  echo "Updater public key not found: $PUBKEY_PATH" >&2
  exit 1
}

[[ -n "$PASSWORD" ]] || {
  echo "SAFECLAW_TAURI_KEY_PASSWORD is required" >&2
  echo "You can export it or keep it in $PASSWORD_FILE_DEFAULT" >&2
  exit 1
}

gh auth status >/dev/null

VERSION="$(
  node -e 'console.log(require(process.argv[1]).version)' "$APP_DIR/package.json"
)"
TAG="${SAFECLAW_RELEASE_TAG:-safeclaw-v$VERSION}"
TITLE="${SAFECLAW_RELEASE_TITLE:-SafeClaw v$VERSION}"
ENDPOINTS="${SAFECLAW_UPDATER_ENDPOINTS:-https://github.com/${REPO}/releases/latest/download/latest.json}"
NOTES_FILE="${SAFECLAW_RELEASE_NOTES_FILE:-}"
PUBLISHED_AT="${SAFECLAW_RELEASE_PUB_DATE:-$(date -u +"%Y-%m-%dT%H:%M:%SZ")}"

TEMP_CONFIG="$APP_DIR/src-tauri/tauri.release.local.json"

cleanup() {
  rm -f "$TEMP_CONFIG"
}
trap cleanup EXIT

PUBKEY_CONTENT="$(cat "$PUBKEY_PATH")"
PRIVATE_KEY_CONTENT="$(cat "$KEY_PATH")"

python3 - "$TEMP_CONFIG" "$PUBKEY_CONTENT" "$ENDPOINTS" <<'PY'
import json
import re
import sys
from pathlib import Path

dest = Path(sys.argv[1])
pubkey = sys.argv[2]
raw_endpoints = sys.argv[3]
endpoints = [part.strip() for part in re.split(r"[,\n;]+", raw_endpoints) if part.strip()]
config = {
    "plugins": {
        "updater": {
            "pubkey": pubkey,
            "endpoints": endpoints,
        }
    }
}
dest.write_text(json.dumps(config, indent=2) + "\n")
PY

export SAFECLAW_UPDATER_PUBKEY="$PUBKEY_CONTENT"
export SAFECLAW_UPDATER_ENDPOINTS="$ENDPOINTS"
export TAURI_SIGNING_PRIVATE_KEY="$PRIVATE_KEY_CONTENT"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$PASSWORD"
export SAFECLAW_REQUIRE_BOX_RESOURCES="${SAFECLAW_REQUIRE_BOX_RESOURCES:-1}"

pushd "$APP_DIR" >/dev/null

if [[ ! -d node_modules ]]; then
  echo "[safeclaw-release] installing frontend dependencies"
  pnpm install --frozen-lockfile
fi

echo "[safeclaw-release] building SafeClaw $VERSION"
if [[ -n "$TARGET" ]]; then
  pnpm tauri build --config src-tauri/tauri.release.local.json --bundles "$BUNDLES" --target "$TARGET"
else
  pnpm tauri build --config src-tauri/tauri.release.local.json --bundles "$BUNDLES"
fi

if [[ "$(uname -s)" == "Darwin" && -f scripts/fix-macos-bundle.mjs ]]; then
  echo "[safeclaw-release] applying macOS bundle fix"
  node scripts/fix-macos-bundle.mjs
fi

echo "[safeclaw-release] generating updater manifest"
python3 - "$REPO" "$TAG" "$VERSION" "$TITLE" "$PUBLISHED_AT" <<'PY'
import json
import sys
from pathlib import Path

repo, tag, version, title, published_at = sys.argv[1:]
root = Path("src-tauri/target/release")
bundle_root = root / "bundle"
platforms = {}

target_triple = None
for candidate in sorted(root.glob("*/release/bundle")):
    try:
        target_triple = candidate.parent.name
        break
    except Exception:
        pass

if target_triple:
    parts = target_triple.split("-")
    if len(parts) >= 3:
        arch = parts[0]
        os_name = parts[2]
    else:
        arch = None
        os_name = None
else:
    arch = None
    os_name = None

if arch == "x86_64":
    updater_arch = "x86_64"
elif arch in {"aarch64", "arm64"}:
    updater_arch = "aarch64"
elif arch == "i686":
    updater_arch = "i686"
elif arch == "armv7":
    updater_arch = "armv7"
elif arch == "riscv64gc":
    updater_arch = "riscv64"
else:
    updater_arch = None

if os_name == "apple":
    updater_os = "darwin"
elif os_name == "windows":
    updater_os = "windows"
elif os_name == "linux":
    updater_os = "linux"
else:
    updater_os = None

if not updater_os or not updater_arch:
    import platform
    machine = platform.machine().lower()
    system = platform.system().lower()
    if not updater_arch:
        updater_arch = {
            "x86_64": "x86_64",
            "amd64": "x86_64",
            "arm64": "aarch64",
            "aarch64": "aarch64",
        }.get(machine)
    if not updater_os:
        updater_os = {
            "darwin": "darwin",
            "linux": "linux",
            "windows": "windows",
        }.get(system)

installer_by_suffix = [
    (".app.tar.gz", "app"),
    (".AppImage.tar.gz", "appimage"),
    (".AppImage", "appimage"),
    (".deb.tar.gz", "deb"),
    (".deb", "deb"),
    (".rpm.tar.gz", "rpm"),
    (".rpm", "rpm"),
    (".nsis.zip", "nsis"),
    (".msi.zip", "msi"),
    (".exe.zip", "nsis"),
    (".exe", "nsis"),
    (".msi", "msi"),
]

def detect_installer(name: str):
    for suffix, installer in installer_by_suffix:
        if name.endswith(suffix):
            return installer
    return None

for asset in sorted(bundle_root.rglob("*")):
    if not asset.is_file():
        continue
    if asset.name.endswith(".sig"):
        continue
    if "SafeClaw" not in asset.name:
        continue
    installer = detect_installer(asset.name)
    if installer is None:
        continue
    sig_path = asset.with_name(asset.name + ".sig")
    if not sig_path.is_file():
        continue
    url = f"https://github.com/{repo}/releases/download/{tag}/{asset.name}"
    signature = sig_path.read_text().strip()
    key = f"{updater_os}-{updater_arch}-{installer}"
    platforms[key] = {
        "url": url,
        "signature": signature,
    }
    fallback_key = f"{updater_os}-{updater_arch}"
    platforms.setdefault(fallback_key, platforms[key])

if not platforms:
    raise SystemExit("No signed updater artifacts found for latest.json generation")

manifest = {
    "version": version,
    "notes": f"{title} local signed release",
    "pub_date": published_at,
    "platforms": platforms,
}
(root / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n")
PY

ASSETS=()
while IFS= read -r asset; do
  [[ -n "$asset" ]] && ASSETS+=("$asset")
done < <(
  python3 - <<'PY'
from pathlib import Path

root = Path("src-tauri/target/release")
allowed_suffixes = (
    ".sig",
    ".dmg",
    ".AppImage",
    ".deb",
    ".rpm",
    ".msi",
    ".exe",
    ".zip",
    ".gz",
)

for path in sorted(root.rglob("*")):
    if not path.is_file():
        continue
    name = path.name
    if name == "latest.json":
        print(path)
        continue
    if not name.startswith("SafeClaw"):
        continue
    if name.endswith(allowed_suffixes):
        print(path)
PY
)

popd >/dev/null

if [[ "${#ASSETS[@]}" -eq 0 ]]; then
  echo "No release assets found after build" >&2
  exit 1
fi

if ! printf '%s\n' "${ASSETS[@]}" | grep -q 'latest.json'; then
  echo "latest.json not found; updater artifacts were not generated" >&2
  printf '%s\n' "${ASSETS[@]}" >&2
  exit 1
fi

for asset in "${ASSETS[@]}"; do
  [[ -f "$APP_DIR/$asset" ]] || {
    echo "Missing asset: $APP_DIR/$asset" >&2
    exit 1
  }
done

if ! gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  if [[ "$CREATE_RELEASE" != "1" ]]; then
    echo "Release $TAG does not exist in $REPO and SAFECLAW_CREATE_RELEASE=0" >&2
    exit 1
  fi

  echo "[safeclaw-release] creating GitHub release $TAG"
  if [[ -n "$NOTES_FILE" ]]; then
    gh release create "$TAG" \
      --repo "$REPO" \
      --title "$TITLE" \
      --notes-file "$NOTES_FILE"
  else
    gh release create "$TAG" \
      --repo "$REPO" \
      --title "$TITLE" \
      --notes "Local SafeClaw desktop release built and signed with the Tauri updater key."
  fi
fi

UPLOAD_ARGS=()
for asset in "${ASSETS[@]}"; do
  UPLOAD_ARGS+=("$APP_DIR/$asset")
done

echo "[safeclaw-release] uploading ${#UPLOAD_ARGS[@]} assets to $REPO release $TAG"
gh release upload "$TAG" --repo "$REPO" --clobber "${UPLOAD_ARGS[@]}"

echo
echo "Done."
echo "Repo:    $REPO"
echo "Tag:     $TAG"
echo "Version: $VERSION"
echo "Release: https://github.com/$REPO/releases/tag/$TAG"
