export type DesktopPlatformId = 'macos-arm64' | 'macos-x64' | 'windows-x64' | 'linux-x64';

export interface DesktopReleaseAsset {
  id: DesktopPlatformId;
  name: string;
  format: string;
  fileName: string;
  href: string;
}

export const desktopRepositoryUrl = 'https://github.com/A3S-Lab/a3s';
export const desktopReleasesUrl = `${desktopRepositoryUrl}/releases`;
export const desktopLatestReleaseTag = 'desktop-latest';
export const desktopLatestReleaseUrl = `${desktopReleasesUrl}/tag/${desktopLatestReleaseTag}`;
export const desktopReleaseAssetBaseUrl = `${desktopReleasesUrl}/download/${desktopLatestReleaseTag}`;
export const desktopChecksumsUrl = `${desktopReleaseAssetBaseUrl}/SHA256SUMS.txt`;

export const desktopReleaseAssets: readonly DesktopReleaseAsset[] = [
  {
    id: 'macos-arm64',
    name: 'macOS (Apple silicon)',
    format: 'DMG',
    fileName: 'A3S-macos-arm64.dmg',
    href: `${desktopReleaseAssetBaseUrl}/A3S-macos-arm64.dmg`,
  },
  {
    id: 'macos-x64',
    name: 'macOS (Intel)',
    format: 'DMG',
    fileName: 'A3S-macos-x64.dmg',
    href: `${desktopReleaseAssetBaseUrl}/A3S-macos-x64.dmg`,
  },
  {
    id: 'windows-x64',
    name: 'Windows (x64)',
    format: 'NSIS installer',
    fileName: 'A3S-windows-x64.exe',
    href: `${desktopReleaseAssetBaseUrl}/A3S-windows-x64.exe`,
  },
  {
    id: 'linux-x64',
    name: 'Linux (x64)',
    format: 'AppImage',
    fileName: 'A3S-linux-x64.AppImage',
    href: `${desktopReleaseAssetBaseUrl}/A3S-linux-x64.AppImage`,
  },
] as const;

export const desktopDownloadContent = {
  en: {
    skip: 'Skip to downloads',
    title: 'Download A3S Desktop',
    description: 'Download the latest A3S Desktop installer from the A3S release.',
    downloadAction: 'Download',
    previewNote: 'Installers are published from the A3S repository release and include SHA-256 checksums.',
    checksumAction: 'SHA-256 checksums',
    releaseNotesAction: 'Release notes',
    sourceAction: 'Source',
    historyTitle: 'Release history and notes',
    historyDescription: 'Review each public release, its changes, and tagged downloads.',
    latestLabel: 'Latest',
    releaseChangesTitle: 'What changed',
    releaseDownloadsTitle: 'Downloads',
    releasePageAction: 'Release page',
    allReleasesAction: 'All releases',
    cliTitle: 'First time using A3S?',
    cliDescription: 'Install and configure the A3S CLI before opening a workspace.',
    installCli: 'Install A3S CLI',
  },
  cn: {
    skip: '跳到下载区域',
    title: '下载 A3S Desktop',
    description: '选择你的系统，从 A3S 主仓库 Release 下载最新版桌面安装包。',
    downloadAction: '下载',
    previewNote: '安装包统一托管在 A3S 主仓库 Release，并提供 SHA-256 校验文件。',
    checksumAction: 'SHA-256 校验',
    releaseNotesAction: '版本说明',
    sourceAction: '源码',
    historyTitle: '历史版本与更新说明',
    historyDescription: '按发布日期查看每个公开版本的变更和对应安装包。',
    latestLabel: '最新',
    releaseChangesTitle: '更新内容',
    releaseDownloadsTitle: '版本下载',
    releasePageAction: '发布页面',
    allReleasesAction: '全部版本',
    cliTitle: '第一次使用 A3S？',
    cliDescription: '打开工作区前，请先安装并配置 A3S CLI。',
    installCli: '安装 A3S CLI',
  },
} as const;
