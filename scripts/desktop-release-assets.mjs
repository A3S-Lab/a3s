import { createHash } from 'node:crypto';
import { realpathSync } from 'node:fs';
import { copyFile, mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

export const DESKTOP_REPOSITORY_URL = 'https://github.com/A3S-Lab/a3s';

/**
 * Stable names exposed by the A3S website. The versioned release keeps the
 * original Tauri assets and receives this alias set as well; the same files
 * are copied to the mutable `desktop-latest` release after every successful
 * Desktop release.
 */
export const DESKTOP_DOWNLOAD_ALIASES = [
  {
    alias: 'A3S-macos-arm64.dmg',
    candidates: [/^A3S-(?:darwin|macos)-(?:aarch64|arm64)\.dmg$/i],
  },
  {
    alias: 'A3S-macos-x64.dmg',
    candidates: [/^A3S-(?:darwin|macos)-(?:x64|x86_64)\.dmg$/i],
  },
  {
    alias: 'A3S-windows-x64.exe',
    candidates: [/^A3S-windows-(?:x64|x86_64)-setup\.exe$/i],
  },
  {
    alias: 'A3S-linux-x64.AppImage',
    candidates: [
      /^A3S-linux-(?:x86_64|amd64)\.AppImage$/i,
      // Keep the historical hyphenated spelling as a migration fallback.
      /^A3S-linux-(?:x86_64|amd64)-AppImage$/i,
    ],
  },
];

function findCandidate(names, patterns) {
  // Pattern order is significant for updater assets: Tauri v2's direct
  // artifact should win over a v1-compatible archive when both are present.
  for (const pattern of patterns) {
    const candidate = names.find((name) => pattern.test(name));
    if (candidate) return candidate;
  }
  return undefined;
}

function updaterSignatureName(assetName) {
  return `${assetName}.sig`;
}

function findSignedCandidate(names, files, patterns) {
  for (const pattern of patterns) {
    const candidate = names.find(
      (name) => pattern.test(name) && files.has(updaterSignatureName(name)),
    );
    if (candidate) return candidate;
  }
  // Return an unsigned candidate as well so the caller can produce the
  // actionable "missing signature" error instead of silently skipping it.
  return findCandidate(names, patterns);
}

/** Return a deterministic source-to-alias mapping and fail closed on gaps. */
export function resolveDesktopDownloadAliases(names) {
  const uniqueNames = [...new Set(names)].sort();
  return DESKTOP_DOWNLOAD_ALIASES.map(({ alias, candidates }) => {
    const source = findCandidate(uniqueNames, candidates);
    if (!source) {
      throw new Error(
        `Desktop release is missing the installer required for ${alias}; available assets: ${uniqueNames.join(', ')}`,
      );
    }
    return { alias, source };
  });
}

const UPDATER_ASSET_SPECS = [
  {
    key: 'darwin-aarch64',
    installer: 'app',
    patterns: [/^A3S-(?:darwin|macos)-(?:aarch64|arm64)\.app\.tar\.gz$/i],
  },
  {
    key: 'darwin-x86_64',
    installer: 'app',
    patterns: [/^A3S-(?:darwin|macos)-(?:x64|x86_64)\.app\.tar\.gz$/i],
  },
  {
    key: 'windows-x86_64',
    installer: 'nsis',
    patterns: [
      /^A3S-windows-(?:x64|x86_64)-setup\.exe$/i,
      /^A3S-windows-(?:x64|x86_64)-setup\.nsis\.zip$/i,
    ],
  },
  {
    key: 'windows-x86_64',
    installer: 'msi',
    patterns: [
      /^A3S-windows-(?:x64|x86_64)\.msi$/i,
      /^A3S-windows-(?:x64|x86_64)\.msi\.zip$/i,
    ],
  },
  {
    key: 'linux-x86_64',
    installer: 'appimage',
    patterns: [
      /^A3S-linux-(?:x86_64|amd64)\.AppImage$/i,
      /^A3S-linux-(?:x86_64|amd64)\.AppImage\.tar\.gz$/i,
    ],
  },
];

/**
 * Rebuild the manifest with public release-download URLs. The Tauri action's
 * API-asset URLs require an API redirect; direct release URLs are cacheable,
 * anonymous, and remain valid when the mutable alias release is refreshed.
 */
export async function buildDesktopUpdaterManifest(
  sourceDirectory,
  releaseTag = 'desktop-latest',
  repositoryUrl = DESKTOP_REPOSITORY_URL,
) {
  if (typeof releaseTag !== 'string' || !releaseTag.trim()) {
    throw new Error('A Desktop release tag is required');
  }
  const tag = releaseTag.trim();
  const raw = JSON.parse(await readFile(path.join(sourceDirectory, 'latest.json'), 'utf8'));
  if (!raw || typeof raw !== 'object' || typeof raw.version !== 'string') {
    throw new Error('Desktop latest.json has no version');
  }
  const tagVersion = /^desktop-v(\d+\.\d+\.\d+)$/u.exec(tag)?.[1];
  const normalizedVersion = raw.version.replace(/^v/iu, '');
  if (tagVersion && tagVersion !== normalizedVersion) {
    throw new Error(`Desktop latest.json version ${raw.version} does not match ${tag}`);
  }
  const entries = await readdir(sourceDirectory, { withFileTypes: true });
  const files = new Set(entries.filter((entry) => entry.isFile()).map((entry) => entry.name));
  const sortedFiles = [...files].sort();
  const platforms = {};
  for (const { key, installer, patterns } of UPDATER_ASSET_SPECS) {
    const assetName = findSignedCandidate(sortedFiles, files, patterns);
    if (!assetName) continue;
    const signatureName = updaterSignatureName(assetName);
    if (!files.has(signatureName)) {
      throw new Error(`Updater asset ${assetName} is missing ${signatureName}`);
    }
    const signature = (await readFile(path.join(sourceDirectory, signatureName), 'utf8')).trim();
    if (!signature) throw new Error(`Updater signature ${signatureName} is empty`);
    const entry = {
      signature,
      url: `${repositoryUrl}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(assetName)}`,
    };
    platforms[key] ??= entry;
    platforms[`${key}-${installer}`] = entry;
  }
  const requiredPlatforms = ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64', 'linux-x86_64'];
  const missing = requiredPlatforms.filter((key) => !platforms[key]);
  if (missing.length) {
    throw new Error(`Desktop release is missing signed updater platforms: ${missing.join(', ')}`);
  }
  return {
    version: normalizedVersion,
    notes: typeof raw.notes === 'string' ? raw.notes : '',
    pub_date: typeof raw.pub_date === 'string' ? raw.pub_date : new Date().toISOString(),
    platforms,
  };
}

async function sha256(filePath) {
  const digest = createHash('sha256').update(await readFile(filePath)).digest('hex');
  return digest;
}

/**
 * Materialize website aliases and a checksum file from a downloaded release.
 * This function is intentionally filesystem-only so it can be tested without
 * GitHub credentials.
 */
export async function materializeDesktopDownloadAliases(
  sourceDirectory,
  outputDirectory,
  releaseTag = 'desktop-latest',
) {
  const names = await readdir(sourceDirectory, { withFileTypes: true });
  const files = names.filter((entry) => entry.isFile()).map((entry) => entry.name);
  const aliases = resolveDesktopDownloadAliases(files);
  if (!files.includes('latest.json')) {
    throw new Error('Desktop release is missing latest.json');
  }

  await mkdir(outputDirectory, { recursive: true });
  const checksums = [];
  for (const { alias, source } of aliases) {
    const target = path.join(outputDirectory, alias);
    await copyFile(path.join(sourceDirectory, source), target);
    checksums.push(`${await sha256(target)}  ${alias}`);
  }
  const manifest = await buildDesktopUpdaterManifest(sourceDirectory, releaseTag);
  await writeFile(path.join(outputDirectory, 'latest.json'), `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  await writeFile(path.join(outputDirectory, 'SHA256SUMS.txt'), `${checksums.join('\n')}\n`, 'utf8');
  return aliases;
}

function isMainModule() {
  if (!process.argv[1]) return false;
  try {
    return realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
  } catch {
    return false;
  }
}

if (isMainModule()) {
  const [sourceDirectory, outputDirectory, releaseTag] = process.argv.slice(2);
  if (!sourceDirectory || !outputDirectory) {
    console.error('Usage: node scripts/desktop-release-assets.mjs <source-directory> <output-directory>');
    process.exitCode = 2;
  } else {
    await materializeDesktopDownloadAliases(sourceDirectory, outputDirectory, releaseTag);
    console.log(`Prepared Desktop download aliases in ${outputDirectory}`);
  }
}
