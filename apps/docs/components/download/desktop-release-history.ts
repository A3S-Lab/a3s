import {
  desktopReleaseAssets,
  desktopRepositoryUrl,
  type DesktopReleaseAsset,
} from './download-content';

type DesktopReleaseLocale = 'cn' | 'en';

export interface DesktopReleaseHistoryEntry {
  version: string;
  publishedAt: string;
  releaseUrl: string;
  checksumUrl: string;
  assets: readonly DesktopReleaseAsset[];
  notes: Readonly<Record<DesktopReleaseLocale, readonly string[]>>;
}

function taggedReleaseAssets(version: string): readonly DesktopReleaseAsset[] {
  const assetBaseUrl = `${desktopRepositoryUrl}/releases/download/${version}`;
  return desktopReleaseAssets.map((asset) => ({
    ...asset,
    href: `${assetBaseUrl}/${asset.fileName}`,
  }));
}

export const desktopReleaseHistory = [
  {
    version: 'v0.1.0',
    publishedAt: '2026-08-23',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/v0.1.0`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/v0.1.0/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('v0.1.0'),
    notes: {
      cn: [
        '首次发布 A3S Desktop 原生工作台，默认进入 Office 模式。',
        '在后台发现本机 A3S CLI，并通过受约束的本地 Host 运行任务。',
        '提供 macOS、Windows 和 Linux 安装包以及 SHA-256 校验文件。',
        '修正 Windows 应用包的归档结构。',
      ],
      en: [
        'Launched the first public A3S Desktop workbench with Office as the default mode.',
        'Discovers the local A3S CLI in the background and runs tasks through the bounded local Host.',
        'Ships macOS, Windows, and Linux packages with a SHA-256 checksum file.',
        'Corrected the Windows application bundle archive structure.',
      ],
    },
  },
] as const satisfies readonly DesktopReleaseHistoryEntry[];
