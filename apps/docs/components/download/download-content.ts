export type DesktopPlatformId = 'macos' | 'windows' | 'linux';

export interface DesktopReleaseAsset {
  id: DesktopPlatformId;
  name: string;
  format: string;
  fileName: string;
  href: string;
}

export const desktopRepositoryUrl = 'https://github.com/A3S-Lab/Desktop';
export const desktopReleasesUrl = `${desktopRepositoryUrl}/releases`;
export const desktopLatestReleaseUrl = `${desktopReleasesUrl}/latest`;
export const desktopReleaseAssetBaseUrl = `${desktopLatestReleaseUrl}/download`;
export const desktopChecksumsUrl = `${desktopReleaseAssetBaseUrl}/SHA256SUMS.txt`;

export const desktopReleaseAssets: readonly DesktopReleaseAsset[] = [
  {
    id: 'macos',
    name: 'macOS',
    format: 'ZIP',
    fileName: 'A3S-macos.zip',
    href: `${desktopReleaseAssetBaseUrl}/A3S-macos.zip`,
  },
  {
    id: 'windows',
    name: 'Windows',
    format: 'ZIP',
    fileName: 'A3S-windows.zip',
    href: `${desktopReleaseAssetBaseUrl}/A3S-windows.zip`,
  },
  {
    id: 'linux',
    name: 'Linux',
    format: 'TAR.GZ',
    fileName: 'A3S-linux.tar.gz',
    href: `${desktopReleaseAssetBaseUrl}/A3S-linux.tar.gz`,
  },
] as const;

export const desktopDownloadContent = {
  en: {
    skip: 'Skip to downloads',
    title: 'Download A3S Desktop',
    description: 'Choose your system and download the latest desktop build.',
    downloadAction: 'Download',
    previewNote: 'Preview builds are currently unsigned.',
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
    description: '选择你的系统，下载最新版桌面应用。',
    downloadAction: '下载',
    previewNote: '当前预览包尚未签名。',
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
