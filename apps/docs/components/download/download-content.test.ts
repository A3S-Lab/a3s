import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  desktopChecksumsUrl,
  desktopDownloadContent,
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

  test('keeps the download page copy focused on choosing a package', () => {
    const expectedKeys = [
      'allReleasesAction',
      'checksumAction',
      'cliDescription',
      'cliTitle',
      'description',
      'downloadAction',
      'historyDescription',
      'historyTitle',
      'installCli',
      'latestLabel',
      'previewNote',
      'releaseChangesTitle',
      'releaseDownloadsTitle',
      'releaseNotesAction',
      'releasePageAction',
      'skip',
      'sourceAction',
      'title',
    ];

    for (const locale of ['cn', 'en'] as const) {
      const content = desktopDownloadContent[locale];
      assert.deepEqual(Object.keys(content).sort(), expectedKeys);
      assert.ok(content.description.length <= 64);
      assert.ok(content.cliDescription.length <= 80);
    }

    assert.equal(desktopDownloadContent.cn.title, '下载 A3S Desktop');
    assert.equal(desktopDownloadContent.en.title, 'Download A3S Desktop');
  });
});
