import {
  File,
  FileArchive,
  FileCode2,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileType2,
  Folder,
  FolderOpen,
  Presentation,
} from 'lucide-react';
import { workFileExtension } from '../work-local-files';

export function WorkFileIcon({
  path,
  directory = false,
  open = false,
  size = 18,
}: {
  path: string;
  directory?: boolean;
  open?: boolean;
  size?: number;
}) {
  if (directory) {
    const Icon = open ? FolderOpen : Folder;
    return (
      <span
        className='work-local-file-icon folder'
        data-compact={size < 24 || undefined}
        style={{ width: size, height: size }}
      >
        <Icon size={size < 24 ? size : Math.round(size * 0.82)} />
      </span>
    );
  }
  const extension = workFileExtension(path);
  const Icon = fileIcon(extension);
  return (
    <span
      className={`work-local-file-icon ${fileTone(extension)}`}
      data-compact={size < 24 || undefined}
      style={{ width: size, height: size }}
    >
      <Icon size={size < 24 ? size : Math.round(size * 0.68)} />
      {size >= 24 && <small>{fileBadge(extension)}</small>}
    </span>
  );
}

function fileIcon(extension: string) {
  if (['doc', 'docx', 'md', 'markdown', 'mdx', 'txt'].includes(extension)) return FileText;
  if (['xls', 'xlsx', 'csv', 'ods'].includes(extension)) return FileSpreadsheet;
  if (['ppt', 'pptx'].includes(extension)) return Presentation;
  if (extension === 'pdf') return FileType2;
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'heic'].includes(extension)) return FileImage;
  if (['zip', '7z', 'rar', 'tar', 'gz'].includes(extension)) return FileArchive;
  if (codeExtensions.has(extension)) return FileCode2;
  return File;
}

function fileTone(extension: string): string {
  if (['doc', 'docx', 'md', 'markdown', 'txt'].includes(extension)) return 'document';
  if (['xls', 'xlsx', 'csv', 'ods'].includes(extension)) return 'spreadsheet';
  if (['ppt', 'pptx'].includes(extension)) return 'presentation';
  if (extension === 'pdf') return 'pdf';
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'heic'].includes(extension)) return 'image';
  if (['zip', '7z', 'rar', 'tar', 'gz'].includes(extension)) return 'archive';
  if (codeExtensions.has(extension)) return 'code';
  return 'default';
}

function fileBadge(extension: string): string {
  if (!extension) return 'FILE';
  if (extension === 'markdown') return 'MD';
  if (extension === 'jpeg') return 'JPG';
  return extension.slice(0, 4).toLocaleUpperCase();
}

const codeExtensions = new Set([
  'c',
  'cc',
  'cpp',
  'cs',
  'css',
  'go',
  'h',
  'hpp',
  'html',
  'htm',
  'java',
  'js',
  'jsx',
  'json',
  'kt',
  'kts',
  'lua',
  'php',
  'py',
  'rb',
  'rs',
  'scss',
  'sh',
  'sql',
  'swift',
  'toml',
  'ts',
  'tsx',
  'vue',
  'xml',
  'yaml',
  'yml',
]);
