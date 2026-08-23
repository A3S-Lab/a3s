import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  desktopChecksumsUrl,
  desktopReleaseAssetBaseUrl,
  desktopReleaseAssets,
  desktopRepositoryUrl,
} from './download-content';

describe('Desktop release links', () => {
  test('provides one stable latest-release asset for every supported platform', () => {
    assert.deepEqual(desktopReleaseAssets.map((asset) => asset.id), ['macos', 'windows', 'linux']);
    assert.equal(new Set(desktopReleaseAssets.map((asset) => asset.fileName)).size, 3);

    for (const asset of desktopReleaseAssets) {
      assert.equal(asset.href, `${desktopReleaseAssetBaseUrl}/${asset.fileName}`);
      assert.equal(
        asset.href.startsWith(`${desktopRepositoryUrl}/releases/latest/download/`),
        true,
      );
    }
  });

  test('publishes a checksum link beside the platform packages', () => {
    assert.equal(
      desktopChecksumsUrl,
      'https://github.com/A3S-Lab/Desktop/releases/latest/download/SHA256SUMS.txt',
    );
  });
});
