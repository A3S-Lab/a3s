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
    version: 'v0.1.2',
    tag: 'desktop-v0.1.2',
    publishedAt: '2026-09-17',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/desktop-v0.1.2`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/desktop-v0.1.2/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('desktop-v0.1.2'),
    notes: {
      cn: [
        '发布 macOS Apple silicon（arm64）桌面安装包与更新通道。',
        '强化 Auto-review × 产物追溯信任环（版本级 Lineage Review 同步）。',
        '专家 Bot 任务审查与 mouse-never 端到端门禁继续以 boyue/deepseek-v4-flash 验证。',
        '下载页仍通过可变的 desktop-latest 别名解析最新安装包。',
      ],
      en: [
        'Ships the macOS Apple silicon (arm64) Desktop installer and updater channel.',
        'Hardens the Auto-review × artifact-lineage trust loop (version-scoped Lineage Review sync).',
        'Keeps expert-bot task-review and mouse-never E2E gates on boyue/deepseek-v4-flash.',
        'The download page still resolves installers through the mutable desktop-latest alias.',
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
