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
    version: 'v0.1.6',
    tag: 'desktop-v0.1.6',
    publishedAt: '2026-09-22',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/desktop-v0.1.6`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/desktop-v0.1.6/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('desktop-v0.1.6'),
    notes: {
      cn: [
        '相对 v0.1.5：单文档 /review 主机续批不再插入多余用户气泡；前文回顾改为右侧异色虚线箭头指向已审段落，且不再自动滚回前文。',
        '批注连线统一为虚线；继续内置 a3s-box 与 a3s-office，Document Expert / 科学家 Bot 开箱可用。',
        'macOS Apple silicon 包仍为 Developer ID 签名并公证；下载页与应用内更新指向本版。',
      ],
      en: [
        'Since v0.1.5: progressive /review host continues no longer paint an extra user bubble; prior-recall uses right-side colored dashed arrows to earlier spans and does not auto-scroll the editor back.',
        'Comment connector leaders are dashed; still ships a3s-box and a3s-office so Document Expert / Scientist Bot work out of the box.',
        'macOS Apple silicon remains Developer ID signed and notarized. Downloads and in-app updates resolve to this build.',
      ],
    },
  },
  {
    version: 'v0.1.5',
    tag: 'desktop-v0.1.5',
    publishedAt: '2026-09-22',
    releaseUrl: `${desktopRepositoryUrl}/releases/tag/desktop-v0.1.5`,
    checksumUrl: `${desktopRepositoryUrl}/releases/download/desktop-v0.1.5/SHA256SUMS.txt`,
    assets: taggedReleaseAssets('desktop-v0.1.5'),
    notes: {
      cn: [
        '相对 v0.1.4：安装包内置 a3s-office，Document Expert（Word / Excel / PPT / Markdown / PDF）开箱可用，无需本机另装 Office CLI。',
        '单文档 /review：渐进批注、Findings 优先队列与 Accept / Address 闭环；复杂合同场景与 live Desktop 门禁已收口。',
        '继续内置 a3s-box（科学家 Bot / 科研环境）；macOS Apple silicon 包仍为 Developer ID 签名并公证，下载页与应用内更新指向本版。',
      ],
      en: [
        'Since v0.1.4: ships bundled a3s-office so Document Expert (Word / Excel / PPT / Markdown / PDF) works out of the box—no separate Office CLI install.',
        'Single-document /review: progressive annotate, Findings priority queue, and Accept / Address loop; complex-contract coverage and live Desktop gates are closed.',
        'Still includes a3s-box for Scientist Bot / Science Lab; macOS Apple silicon remains Developer ID signed and notarized. Downloads and in-app updates resolve to this build.',
      ],
    },
  },
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
