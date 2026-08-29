export type DesktopPlatformId = "macos" | "windows" | "linux";

export interface DesktopReleaseAsset {
  id: DesktopPlatformId;
  name: string;
  architecture: "arm64" | "x86-64";
  format: string;
  fileName: string;
  href: string;
}

export const desktopRepositoryUrl = "https://github.com/A3S-Lab/Desktop";
export const desktopReleasesUrl = `${desktopRepositoryUrl}/releases`;
export const desktopLatestReleaseUrl = `${desktopReleasesUrl}/latest`;
export const desktopReleaseAssetBaseUrl = `${desktopLatestReleaseUrl}/download`;
export const desktopChecksumsUrl =
  `${desktopReleaseAssetBaseUrl}/SHA256SUMS.txt`;

export const desktopReleaseAssets: readonly DesktopReleaseAsset[] = [
  {
    id: "macos",
    name: "macOS",
    architecture: "arm64",
    format: "ZIP",
    fileName: "A3S-macos.zip",
    href: `${desktopReleaseAssetBaseUrl}/A3S-macos.zip`,
  },
  {
    id: "windows",
    name: "Windows",
    architecture: "x86-64",
    format: "ZIP",
    fileName: "A3S-windows.zip",
    href: `${desktopReleaseAssetBaseUrl}/A3S-windows.zip`,
  },
  {
    id: "linux",
    name: "Linux",
    architecture: "x86-64",
    format: "TAR.GZ",
    fileName: "A3S-linux.tar.gz",
    href: `${desktopReleaseAssetBaseUrl}/A3S-linux.tar.gz`,
  },
] as const;

export const desktopDownloadContent = {
  en: {
    skip: "Skip to downloads",
    title: "Download A3S.",
    description: "Get A3S for macOS, Windows, or Linux.",
    availability: "Preview packages are currently unsigned.",
    downloadAction: "Download",
    checksumAction: "SHA-256 checksums",
    releaseNotesAction: "Release notes",
    sourceAction: "Source",
  },
  cn: {
    skip: "跳到下载区域",
    title: "下载 A3S。",
    description: "获取适用于 macOS、Windows 或 Linux 的 A3S。",
    availability: "当前预览安装包尚未签名。",
    downloadAction: "下载",
    checksumAction: "SHA-256 校验和",
    releaseNotesAction: "版本说明",
    sourceAction: "源代码",
  },
} as const;
