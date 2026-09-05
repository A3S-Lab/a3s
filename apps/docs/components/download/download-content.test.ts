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
    assert.deepEqual(desktopReleaseAssets.map((asset) => asset.id), [
      'macos-arm64',
      'macos-x64',
      'windows-x64',
      'linux-x64',
    ]);
    assert.equal(new Set(desktopReleaseAssets.map((asset) => asset.fileName)).size, 4);

    for (const asset of desktopReleaseAssets) {
      assert.equal(asset.href, `${desktopReleaseAssetBaseUrl}/${asset.fileName}`);
      assert.equal(
        asset.href.startsWith(`${desktopRepositoryUrl}/releases/download/desktop-latest/`),
        true,
      );
    }
  });

  test('publishes a checksum link beside the platform packages', () => {
    assert.equal(
      desktopChecksumsUrl,
      'https://github.com/A3S-Lab/a3s/releases/download/desktop-latest/SHA256SUMS.txt',
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
