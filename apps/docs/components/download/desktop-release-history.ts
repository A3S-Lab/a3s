import {
  desktopReleaseAssets,
  desktopRepositoryUrl,
  type DesktopReleaseAsset,
} from './download-content';

type DesktopReleaseLocale = 'cn' | 'en';

export interface DesktopReleaseHistoryEntry {
  version: string;
  tag: string;
  publishedAt: string;
  releaseUrl: string;
  checksumUrl: string;
  assets: readonly DesktopReleaseAsset[];
  notes: Readonly<Record<DesktopReleaseLocale, readonly string[]>>;
}

function taggedReleaseAssets(tag: string): readonly DesktopReleaseAsset[] {
  const assetBaseUrl = `${desktopRepositoryUrl}/releases/download/${tag}`;
  return desktopReleaseAssets.map((asset) => ({
    ...asset,
    href: `${assetBaseUrl}/${asset.fileName}`,
  }));
}

export const desktopReleaseHistory = [
  {
    version: 'v0.1.4',
    tag: 'desktop-v0.1.4',
    publishedAt: '2026-09-20',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/desktop-v0.1.4`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/desktop-v0.1.4/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('desktop-v0.1.4'),
    notes: {
      cn: [
        'macOS Apple silicon 安装包已 Developer ID 签名并公证，下载后可直接打开安装。',
        '内置 a3s-box 3.2.7，科学家 Bot / 科研环境无需本机另行安装虚拟机运行时。',
        '应用内更新通道与下载页均通过 desktop-latest 指向本版本。',
      ],
      en: [
        'macOS Apple silicon build is Developer ID signed and notarized; Gatekeeper allows open-and-install.',
        'Ships a3s-box 3.2.7 so Scientist Bot / Science Lab needs no separate host runtime install.',
        'In-app updater and the download page resolve this build through desktop-latest.',
      ],
    },
  },
  {
    version: 'v0.1.3',
    tag: 'desktop-v0.1.3',
    publishedAt: '2026-09-18',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/desktop-v0.1.3`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/desktop-v0.1.3/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('desktop-v0.1.3'),
    notes: {
      cn: [
        '发布 macOS Apple silicon 安装包与应用内更新归档。',
        '下载页与更新通道通过 desktop-latest 解析到本版本。',
        '该构建为临时签名，未做 Apple 公证（请改用 v0.1.4）。',
      ],
      en: [
        'Published the macOS Apple silicon installer and in-app updater archive.',
        'Website downloads and the updater resolved this build through desktop-latest.',
        'This build was ad-hoc signed and not notarized (use v0.1.4 instead).',
      ],
    },
  },
  {
    version: 'v0.1.2',
    tag: 'desktop-v0.1.2',
    publishedAt: '2026-09-17',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/desktop-v0.1.2`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/desktop-v0.1.2/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('desktop-v0.1.2'),
    notes: {
      cn: [
        '补充 macOS Apple silicon 安装包引导发布。',
        '网站下载仍通过可变别名 desktop-latest 获取最新包。',
        '提供 SHA-256 校验文件便于核对安装包完整性。',
      ],
      en: [
        'Bootstrap macOS Apple silicon Desktop installer release.',
        'Website downloads continue to resolve through the mutable desktop-latest alias.',
        'Includes a SHA-256 checksum file for installer verification.',
      ],
    },
  },
  {
    version: 'v0.1.0',
    tag: 'desktop-v0.1.0',
    publishedAt: '2026-08-23',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/desktop-v0.1.0`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/desktop-v0.1.0/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('desktop-v0.1.0'),
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
