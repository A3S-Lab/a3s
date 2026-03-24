# SafeClaw Local Release

Use `scripts/safeclaw-local-release.sh` to build SafeClaw locally and upload the generated signed binaries and updater metadata directly to GitHub Releases without committing SafeClaw source code.

## Defaults

- App directory: `apps/safeclaw`
- Release repo: `A3S-Lab/SafeClaw`
- Tag: `safeclaw-v<package.json version>`
- Updater endpoint: `https://github.com/A3S-Lab/SafeClaw/releases/latest/download/latest.json`
- Signing key: `~/.tauri/safeclaw-updater.key`
- Password fallback file: `/tmp/safeclaw_tauri_key_password.txt`

## Required Preconditions

- `gh auth status` succeeds
- Tauri updater keypair exists locally
- `SAFECLAW_TAURI_KEY_PASSWORD` is exported, or `/tmp/safeclaw_tauri_key_password.txt` exists

## Basic Usage

```bash
export SAFECLAW_TAURI_KEY_PASSWORD='your-password'
scripts/safeclaw-local-release.sh
```

## Optional Variables

- `SAFECLAW_RELEASE_REPO`
- `SAFECLAW_RELEASE_TAG`
- `SAFECLAW_RELEASE_TITLE`
- `SAFECLAW_RELEASE_NOTES_FILE`
- `SAFECLAW_BUILD_TARGET`
- `SAFECLAW_UPDATER_ENDPOINTS`
- `SAFECLAW_APP_DIR`
- `SAFECLAW_CREATE_RELEASE=0`

## What It Does

1. Generates a temporary Tauri updater config with the local public key.
2. Builds SafeClaw locally with updater artifact generation enabled.
3. Applies the macOS bundle fix when running on macOS.
4. Collects `latest.json`, signatures, and platform bundles from `src-tauri/target/release`.
5. Creates the GitHub Release if missing.
6. Uploads assets with `gh release upload --clobber`.
