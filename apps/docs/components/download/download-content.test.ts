import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  desktopChecksumsUrl,
  desktopDownloadContent,
  desktopReleaseAssetBaseUrl,
  desktopReleaseAssets,
  desktopRepositoryUrl,
} from "./download-content";

describe("Desktop release links", () => {
  test("provides one stable latest-release asset for every supported platform", () => {
    assert.deepEqual(desktopReleaseAssets.map((asset) => asset.id), [
      "macos",
      "windows",
      "linux",
    ]);
    assert.equal(
      new Set(desktopReleaseAssets.map((asset) => asset.fileName)).size,
      3,
    );

    for (const asset of desktopReleaseAssets) {
      assert.equal(
        asset.href,
        `${desktopReleaseAssetBaseUrl}/${asset.fileName}`,
      );
      assert.equal(
        asset.href.startsWith(
          `${desktopRepositoryUrl}/releases/latest/download/`,
        ),
        true,
      );
    }

    assert.deepEqual(
      desktopReleaseAssets.map(({ id, architecture }) => [id, architecture]),
      [["macos", "arm64"], ["windows", "x86-64"], ["linux", "x86-64"]],
    );
  });

  test("publishes a checksum link beside the platform packages", () => {
    assert.equal(
      desktopChecksumsUrl,
      "https://github.com/A3S-Lab/Desktop/releases/latest/download/SHA256SUMS.txt",
    );
  });

  test("presents A3S as the downloaded product in both languages", () => {
    assert.equal(desktopDownloadContent.en.title, "Download A3S.");
    assert.equal(desktopDownloadContent.cn.title, "下载 A3S。");
    assert.doesNotMatch(JSON.stringify(desktopDownloadContent), /A3S Code/u);
  });

  test("uses A3S package names for every platform", () => {
    assert.equal(
      desktopReleaseAssets.every((asset) => asset.fileName.startsWith("A3S-")),
      true,
    );
  });
});
