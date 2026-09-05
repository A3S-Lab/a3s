import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { describe, test } from 'node:test';
import {
  buildDesktopUpdaterManifest,
  materializeDesktopDownloadAliases,
  resolveDesktopDownloadAliases,
} from './desktop-release-assets.mjs';

const sourceNames = [
  'A3S-darwin-aarch64.dmg',
  'A3S-darwin-x64.dmg',
  'A3S-darwin-aarch64.app.tar.gz',
  'A3S-darwin-aarch64.app.tar.gz.sig',
  'A3S-darwin-x64.app.tar.gz',
  'A3S-darwin-x64.app.tar.gz.sig',
  'A3S-windows-x64-setup.exe',
  'A3S-windows-x64-setup.exe.sig',
  'A3S-windows-x64-setup.nsis.zip',
  'A3S-windows-x64-setup.nsis.zip.sig',
  'A3S-windows-x64.msi',
  'A3S-windows-x64.msi.sig',
  'A3S-windows-x64.msi.zip',
  'A3S-windows-x64.msi.zip.sig',
  'A3S-linux-x86_64.AppImage',
  'A3S-linux-x86_64.AppImage.sig',
  'A3S-linux-x86_64.AppImage.tar.gz',
  'A3S-linux-x86_64.AppImage.tar.gz.sig',
  'latest.json',
];

describe('Desktop release aliases', () => {
  test('resolves every website installer to a versioned Tauri asset', () => {
    assert.deepEqual(resolveDesktopDownloadAliases(sourceNames), [
      { alias: 'A3S-macos-arm64.dmg', source: 'A3S-darwin-aarch64.dmg' },
      { alias: 'A3S-macos-x64.dmg', source: 'A3S-darwin-x64.dmg' },
      { alias: 'A3S-windows-x64.exe', source: 'A3S-windows-x64-setup.exe' },
      { alias: 'A3S-linux-x64.AppImage', source: 'A3S-linux-x86_64.AppImage' },
    ]);
  });

  test('fails when a platform installer or updater manifest is missing', async () => {
    assert.throws(
      () => resolveDesktopDownloadAliases(['A3S-macos-aarch64.dmg']),
      /missing the installer required for A3S-macos-x64\.dmg/,
    );
    const source = await mkdtemp(path.join(os.tmpdir(), 'a3s-desktop-release-source-'));
    const output = await mkdtemp(path.join(os.tmpdir(), 'a3s-desktop-release-output-'));
    try {
      await writeFile(path.join(source, 'A3S-darwin-aarch64.dmg'), 'arm');
      await writeFile(path.join(source, 'A3S-darwin-x64.dmg'), 'x64');
      await writeFile(path.join(source, 'A3S-windows-x64-setup.exe'), 'windows');
      await writeFile(path.join(source, 'A3S-linux-x86_64.AppImage'), 'linux');
      await assert.rejects(
        materializeDesktopDownloadAliases(source, output),
        /missing latest\.json/,
      );
    } finally {
      await Promise.all([rm(source, { recursive: true, force: true }), rm(output, { recursive: true, force: true })]);
    }
  });

  test('materializes aliases, preserves latest.json, and writes checksums', async () => {
    const source = await mkdtemp(path.join(os.tmpdir(), 'a3s-desktop-release-source-'));
    const output = await mkdtemp(path.join(os.tmpdir(), 'a3s-desktop-release-output-'));
    try {
      for (const name of sourceNames) {
        await writeFile(
          path.join(source, name),
          name === 'latest.json'
            ? '{"version":"v0.1.0","notes":"test","pub_date":"2026-09-02T00:00:00.000Z"}'
            : name.endsWith('.sig')
            ? `signature-${name}`
            : name,
        );
      }
      await materializeDesktopDownloadAliases(source, output, 'desktop-v0.1.0');
      assert.equal(await readFile(path.join(output, 'A3S-macos-arm64.dmg'), 'utf8'), 'A3S-darwin-aarch64.dmg');
      const manifest = JSON.parse(await readFile(path.join(output, 'latest.json'), 'utf8'));
      assert.equal(manifest.version, '0.1.0');
      assert.equal(
        manifest.platforms['darwin-aarch64'].url,
        'https://github.com/A3S-Lab/a3s/releases/download/desktop-v0.1.0/A3S-darwin-aarch64.app.tar.gz',
      );
      assert.equal(
        manifest.platforms['windows-x86_64'].url,
        'https://github.com/A3S-Lab/a3s/releases/download/desktop-v0.1.0/A3S-windows-x64-setup.exe',
      );
      assert.equal(
        manifest.platforms['windows-x86_64-msi'].url,
        'https://github.com/A3S-Lab/a3s/releases/download/desktop-v0.1.0/A3S-windows-x64.msi',
      );
      assert.equal(
        manifest.platforms['linux-x86_64'].url,
        'https://github.com/A3S-Lab/a3s/releases/download/desktop-v0.1.0/A3S-linux-x86_64.AppImage',
      );
      for (const key of [
        'darwin-aarch64',
        'darwin-x86_64',
        'darwin-aarch64-app',
        'darwin-x86_64-app',
        'windows-x86_64',
        'windows-x86_64-nsis',
        'windows-x86_64-msi',
        'linux-x86_64',
        'linux-x86_64-appimage',
      ]) {
        assert.equal(typeof manifest.platforms[key]?.signature, 'string', key);
        assert.ok(manifest.platforms[key].signature.length > 0, key);
      }
      const checksums = await readFile(path.join(output, 'SHA256SUMS.txt'), 'utf8');
      assert.match(checksums, /A3S-macos-arm64\.dmg/);
      assert.match(checksums, /A3S-linux-x64\.AppImage/);
    } finally {
      await Promise.all([rm(source, { recursive: true, force: true }), rm(output, { recursive: true, force: true })]);
    }
  });

  test('falls back to a signed archive when a direct v2 artifact has no signature', async () => {
    const source = await mkdtemp(path.join(os.tmpdir(), 'a3s-desktop-release-source-'));
    const names = [
      'A3S-darwin-aarch64.app.tar.gz',
      'A3S-darwin-aarch64.app.tar.gz.sig',
      'A3S-darwin-x64.app.tar.gz',
      'A3S-darwin-x64.app.tar.gz.sig',
      'A3S-windows-x64-setup.exe',
      'A3S-windows-x64-setup.nsis.zip',
      'A3S-windows-x64-setup.nsis.zip.sig',
      'A3S-linux-amd64.AppImage',
      'A3S-linux-amd64.AppImage.tar.gz',
      'A3S-linux-amd64.AppImage.tar.gz.sig',
      'latest.json',
    ];
    try {
      for (const name of names) {
        await writeFile(
          path.join(source, name),
          name === 'latest.json'
            ? '{"version":"0.1.0"}'
            : name.endsWith('.sig')
            ? `signature-${name}`
            : name,
        );
      }
      const manifest = await buildDesktopUpdaterManifest(source, 'desktop-v0.1.0');
      assert.match(manifest.platforms['windows-x86_64'].url, /setup\.nsis\.zip$/u);
      assert.match(manifest.platforms['linux-x86_64'].url, /\.AppImage\.tar\.gz$/u);
    } finally {
      await rm(source, { recursive: true, force: true });
    }
  });

  test('rejects a manifest whose version differs from the release tag', async () => {
    const source = await mkdtemp(path.join(os.tmpdir(), 'a3s-desktop-release-source-'));
    const output = await mkdtemp(path.join(os.tmpdir(), 'a3s-desktop-release-output-'));
    try {
      for (const name of sourceNames) {
        await writeFile(
          path.join(source, name),
          name === 'latest.json'
            ? '{"version":"9.9.9"}'
            : name.endsWith('.sig')
            ? `signature-${name}`
            : name,
        );
      }
      await assert.rejects(
        materializeDesktopDownloadAliases(source, output, 'desktop-v0.1.0'),
        /does not match desktop-v0\.1\.0/,
      );
    } finally {
      await Promise.all([rm(source, { recursive: true, force: true }), rm(output, { recursive: true, force: true })]);
    }
  });
});
