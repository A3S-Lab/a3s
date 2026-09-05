# A3S Desktop release and update contract

The A3S monorepo is the distribution owner for A3S Desktop. `apps/desktop` remains the application
source, while the root repository publishes the installers and signed Tauri updater artifacts.

## Release layout

| Resource | Contract |
| --- | --- |
| Versioned release | `https://github.com/A3S-Lab/a3s/releases/tag/desktop-vX.Y.Z` (native assets plus stable aliases/checksums) |
| Versioned tag | `desktop-vX.Y.Z`; it must match the npm, Tauri, and Cargo Desktop versions. |
| Mutable download release | `desktop-latest` (never use the mixed-product `/releases/latest` endpoint). |
| Updater manifest | `desktop-latest/latest.json`; signed entries point to the versioned release. |
| Website page | `https://a3s-lab.github.io/a3s/download/`; links resolve to `desktop-latest`. |

The release workflow builds macOS arm64 and x64 app/DMG bundles, Windows x64 NSIS/MSI installers,
and Linux x64 AppImage/DEB packages. With Tauri v2 updater artifacts enabled, macOS publishes a
signed `.app.tar.gz`; Linux publishes a signed self-contained `.AppImage`; and Windows publishes
signed self-contained `.exe` and `.msi` artifacts. The verifier also accepts the older
`.AppImage.tar.gz`, `.nsis.zip`, and `.msi.zip` forms so a migration can be diagnosed without
silently accepting an unsigned or cryptographically invalid file. After all matrix jobs finish,
`scripts/desktop-release-assets.mjs` verifies the required files, rewrites updater URLs to public
versioned-release downloads, creates deterministic installer aliases, and writes `SHA256SUMS.txt`.
The aliases and checksum file are mirrored to both the immutable versioned release (for release
history links) and `desktop-latest` (for the download page).

The build is deliberately split into a source gate, a native bundle build, and a package gate:

1. `npm ci` installs the locked Node 26 toolchain and `npm run check` validates the frontend,
   production dependency audit, native sandbox contract, and locked Rust crate.
2. `npm run package:<platform>` invokes Tauri v2 with an explicit bundle set. The Tauri
   `beforeBuildCommand` rebuilds the frontend and checks the native sandbox dependency before Rust
   compilation.
3. `npm run verify:package -- --bundles ...` inspects the files under
   `src-tauri/target/release/bundle`, checks versions and resources, rejects legacy sidecars, and
   verifies macOS signatures/DMGs when run on macOS. Release jobs add `--require-updater --verify-signatures`
   to require a cryptographically valid signature for each platform updater candidate, preferring
   the Tauri v2 direct form and retaining a signed v1-compatible archive as a migration fallback.

For a local macOS build:

```bash
cd apps/desktop
npm ci
npm run check
npm run package:macos
npm run verify:package -- --bundles app,dmg
```

`npm run package:local` is the convenience command that builds the current platform and runs the
package verifier. The generated `src-tauri/tauri.release.conf.json` is ignored and should only be
created by `npm run release:config` in a release job; the private signing key is read from
`TAURI_SIGNING_PRIVATE_KEY` and is never written to disk.

`a3s-updater` remains the CLI self-update library. It resolves a CLI tarball from the GitHub API and
atomically replaces one executable; it does not understand Tauri bundle signatures, app bundles,
native installers, or the platform-specific restart rules required by Desktop. Desktop therefore
uses the official Tauri updater plugin for the app transaction while sharing the root repository's
release ownership and signing policy.

## In-app lifecycle

1. The About panel checks `desktop-latest/latest.json` only in a native Tauri webview, with a bounded
   15-second check timeout. Browser previews remain explicitly unsupported and never call the updater.
2. Tauri verifies the minisign signature using the public key embedded in `tauri.conf.json` (release
   builds override it with the repository secret).
3. When a newer version is available, the panel displays its version and bounded release notes.
4. Choosing **Install and Restart** downloads the signed updater artifact (with a ten-minute download
   timeout), reports progress, installs it, and immediately requests a process restart. Windows' installer is also passed
   `restartAfterInstall=true`; macOS/Linux use `tauri-plugin-process` after installation.
5. A failed check or install does not discard the downloaded update handle and exposes a retry. No
   unsigned or partially downloaded artifact is launched.

Until the first `desktop-vX.Y.Z` workflow has populated the mutable release, GitHub returns 404 for
`desktop-latest/latest.json`. Desktop renders that expected bootstrap state as “update feed not yet
published”; network, signature, and install errors remain actionable failures.

The updater endpoint is HTTPS-only and intentionally does not use GitHub's mutable `latest` release:
the root repository also publishes CLI and other product releases.

## Repository secrets

Configure these secrets before pushing a `desktop-vX.Y.Z` tag:

- `A3S_DESKTOP_UPDATER_PUBKEY`: the minisign public key committed in the Desktop updater config;
- `A3S_DESKTOP_TAURI_SIGNING_PRIVATE_KEY`: the matching private key;
- `A3S_DESKTOP_TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: optional private-key password.

That key signs Tauri updater files (the adjacent `.sig` files). macOS application code signing and notarization are
separate Apple credentials; without them, a local or CI build remains ad-hoc signed even though its
updater artifact can be minisign-signed. Add and verify the Apple credentials before calling a build
distribution-ready.

For a local signed dry run, set `A3S_DESKTOP_UPDATER_PUBKEY` to the committed public key and set
`TAURI_SIGNING_PRIVATE_KEY` to the matching key content or its file path. Generate the ignored
release overlay before invoking the wrapper:

```bash
cd apps/desktop
export A3S_DESKTOP_UPDATER_PUBKEY="$(sed -n 's/.*\"pubkey\": \"\([^\"]*\)\".*/\1/p' src-tauri/tauri.conf.json)"
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/a3s-desktop.key"
npm run release:config
npm run release:build -- build --config src-tauri/tauri.release.conf.json \
  --target aarch64-apple-darwin --bundles app
```

The release wrapper also accepts `TAURI_SIGNING_PRIVATE_KEY_PATH` and normalizes it for the Tauri
CLI. Never commit either key form or the generated overlay.

The private key must be retained outside Git. Rotating it requires shipping a Desktop build with the
new public key before publishing artifacts signed by the new key; losing it prevents installed
versions from accepting future updates.

## Release checklist

1. In the Desktop repository, update the npm, Tauri, and Cargo versions together and refresh
   `package-lock.json`. Run the source gate and the platform package command locally.
2. Publish that Desktop commit, then update the `apps/desktop` gitlink in the A3S root repository.
   The root tag must point at the exact reviewed gitlink; a tag that references an unpushed
   submodule commit cannot be built by GitHub Actions.
3. Confirm the three updater secrets above, then create and push an annotated root tag:

   ```bash
   git tag -a desktop-vX.Y.Z -m "A3S Desktop vX.Y.Z"
   git push origin desktop-vX.Y.Z
   ```

4. The `desktop-release.yml` workflow runs the source gate, builds native macOS arm64/x64,
   Linux x64, and Windows x64 bundles serially, verifies a signed updater candidate for each
   platform, and publishes the versioned assets.
5. The final job materializes aliases and `SHA256SUMS.txt`, mirrors them to the versioned and
   `desktop-latest` releases, then fetches the public `latest.json` feed and checks its version,
   signatures, and immutable-release URLs.

Do not manually upload a replacement `latest.json`: it must be generated from the same release
assets that passed the package verifier.

## Local verification

From `apps/desktop`:

```bash
npm test -- --run scripts/verify-package.test.ts scripts/prepare-release-config.test.ts
npm test -- --run src/features/settings/model/desktop-updater.test.ts src/shell/dialogs.test.tsx
npm run typecheck
npm run build
npm run verify:package -- --bundles app,dmg
cargo check --manifest-path src-tauri/Cargo.toml --locked
```

From the monorepo root, the package and release helpers are also exposed as
`just desktop-package`, `just desktop-package-verify`, `just desktop-release-config`, and
`just desktop-release-build build --config src-tauri/tauri.release.conf.json --target
aarch64-apple-darwin --bundles app`.

From the root repository:

```bash
node --test scripts/desktop-release-assets.test.mjs
```

The release workflow repeats the alias contract test before touching GitHub Releases. It never
replaces the native versioned artifacts; only the deterministic alias/checksum set may be refreshed
on a rerun, while `desktop-latest` remains the mutable download surface.
