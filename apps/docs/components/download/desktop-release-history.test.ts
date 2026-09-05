import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { desktopRepositoryUrl } from './download-content';
import { desktopReleaseHistory } from './desktop-release-history';

describe('Desktop release history', () => {
  test('lists checked releases from newest to oldest', () => {
    assert.ok(desktopReleaseHistory.length > 0);
    assert.equal(desktopReleaseHistory[0]?.version, 'v0.1.0');
    assert.equal(desktopReleaseHistory[0]?.tag, 'desktop-v0.1.0');

    const publishedDates = desktopReleaseHistory.map((release) => release.publishedAt);
    assert.deepEqual(publishedDates, [...publishedDates].sort().reverse());
    assert.equal(new Set(desktopReleaseHistory.map((release) => release.version)).size, desktopReleaseHistory.length);
  });

  test('keeps tagged downloads and release notes with every version', () => {
    for (const release of desktopReleaseHistory) {
      const assetBase = `${desktopRepositoryUrl}/releases/download/${release.tag}`;
      assert.equal(release.releaseUrl, `${desktopRepositoryUrl}/releases/tag/${release.tag}`);
      assert.equal(release.checksumUrl, `${assetBase}/SHA256SUMS.txt`);
      assert.deepEqual(release.assets.map((asset) => asset.id), [
        'macos-arm64',
        'macos-x64',
        'windows-x64',
        'linux-x64',
      ]);

      for (const asset of release.assets) {
        assert.equal(asset.href, `${assetBase}/${asset.fileName}`);
      }

      assert.ok(release.notes.cn.length >= 3);
      assert.ok(release.notes.en.length >= 3);
      assert.equal(release.notes.cn.every(Boolean), true);
      assert.equal(release.notes.en.every(Boolean), true);
    }
  });
});
