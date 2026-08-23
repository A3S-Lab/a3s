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
    title: 'A3S Code, built for your desktop.',
    description:
      'A native workbench for office, coding, and design workflows, powered by the same local A3S Code kernel. Start with Office today; Code and Design arrive as verified contracts are completed.',
    releaseAction: 'View release history',
    sourceAction: 'View source',
    availability: 'macOS · Windows · Linux',
    downloadHeading: 'Download the latest release',
    downloadDescription:
      'Choose your platform. Every package is produced by the tagged Desktop release workflow. Current preview packages are unsigned.',
    downloadAction: 'Download',
    checksumAction: 'Verify SHA-256 checksums',
    releaseNotesAction: 'Release notes',
    requirementsTitle: 'Before you open A3S',
    requirementsDescription:
      'Desktop discovers A3S Code locally and keeps execution policy with the installed CLI.',
    requirements: [
      [
        'Install A3S Code',
        'Install the A3S CLI and configure a supported model provider before starting work.',
      ],
      [
        'Open a workspace',
        'A3S validates the local workspace, ACL path, model, mode, and tool policy before execution.',
      ],
      [
        'Keep authority explicit',
        'Office, Code, and Design never elevate permissions when you switch work modes.',
      ],
    ],
    installCli: 'Install the A3S CLI',
    trustTitle: 'Local-first by construction.',
    trustDescription:
      'The React interface talks to an authenticated loopback Host. The Host launches shell-free A3S Code commands and awaits every child process after completion, failure, or cancellation.',
    trustPoints: [
      'Authenticated loopback API',
      'Typed A3S Code contracts',
      'Versioned GitHub releases',
    ],
    footerDescription: 'The native workbench for A3S Code.',
    footerHome: 'A3S ecosystem',
    footerReleases: 'All releases',
    footerSource: 'Source',
  },
  cn: {
    skip: '跳到下载区域',
    title: '把 A3S Code 带到桌面。',
    description:
      '由同一个本地 A3S Code 内核驱动的原生工作台，覆盖办公、编码与设计三种工作方式。办公模式现已可用，编码和设计会在真实协议完成后开放。',
    releaseAction: '查看发布记录',
    sourceAction: '查看源码',
    availability: 'macOS · Windows · Linux',
    downloadHeading: '下载最新版本',
    downloadDescription:
      '选择你的平台。每个安装包都由带标签的 Desktop 发布工作流生成。当前预览包尚未签名。',
    downloadAction: '下载',
    checksumAction: '校验 SHA-256',
    releaseNotesAction: '版本说明',
    requirementsTitle: '打开 A3S 前',
    requirementsDescription: 'Desktop 在本机发现 A3S Code，执行策略始终由已安装的 CLI 管理。',
    requirements: [
      ['安装 A3S Code', '先安装 A3S CLI，并配置受支持的模型 Provider。'],
      ['打开工作区', '执行前会校验本地工作区、ACL 路径、模型、运行模式和工具策略。'],
      ['权限保持明确', '切换办公、编码或设计模式时，A3S 不会自动提高权限。'],
    ],
    installCli: '安装 A3S CLI',
    trustTitle: '从底层开始坚持本地优先。',
    trustDescription:
      'React 界面只连接带认证的本机 Host。Host 以无 Shell 参数启动 A3S Code，并在完成、失败或取消后等待每个子进程退出。',
    trustPoints: ['带认证的本机 API', '类型化 A3S Code 协议', '版本化 GitHub Releases'],
    footerDescription: 'A3S Code 的原生桌面工作台。',
    footerHome: 'A3S 生态',
    footerReleases: '全部版本',
    footerSource: '源代码',
  },
} as const;
