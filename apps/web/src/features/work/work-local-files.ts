import type { WorkspaceEntry } from '../../types/api';

export type WorkFilesLayout = 'grid' | 'list';
export type WorkFilesSortKey = 'name' | 'modified' | 'size' | 'kind';
export type WorkFilesSortDirection = 'ascending' | 'descending';

export interface WorkFilesSort {
  key: WorkFilesSortKey;
  direction: WorkFilesSortDirection;
}

export interface WorkBreadcrumb {
  label: string;
  path: string;
}

export const WORK_LOCAL_FILE_DRAG_TYPE = 'application/x-a3s-work-local-paths';

const importableExtensions = new Set([
  'docx',
  'xlsx',
  'xls',
  'csv',
  'ods',
  'pptx',
  'pdf',
  'html',
  'htm',
  'txt',
  'md',
  'markdown',
]);

const officeEditorExtensions = new Set(['docx', 'xlsx', 'xls', 'csv', 'ods', 'pptx', 'pdf']);

const collator = new Intl.Collator('zh-CN', {
  numeric: true,
  sensitivity: 'base',
});

export function normalizeLocalPath(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  if (normalized === '/' || /^[A-Za-z]:\/$/.test(normalized)) return normalized;
  return normalized.replace(/\/+$/, '');
}

export function sameLocalPath(left: string, right: string): boolean {
  const normalizedLeft = normalizeLocalPath(left);
  const normalizedRight = normalizeLocalPath(right);
  const windows = /^[A-Za-z]:\//.test(normalizedLeft) || /^[A-Za-z]:\//.test(normalizedRight);
  return windows
    ? normalizedLeft.toLocaleLowerCase() === normalizedRight.toLocaleLowerCase()
    : normalizedLeft === normalizedRight;
}

export function localPathInside(parent: string, candidate: string): boolean {
  const normalizedParent = normalizeLocalPath(parent);
  const normalizedCandidate = normalizeLocalPath(candidate);
  if (!normalizedParent || !normalizedCandidate) return false;
  if (normalizedParent === '/') return normalizedCandidate.startsWith('/');
  const windows = /^[A-Za-z]:\//.test(normalizedParent);
  const base = windows ? normalizedParent.toLocaleLowerCase() : normalizedParent;
  const value = windows ? normalizedCandidate.toLocaleLowerCase() : normalizedCandidate;
  return value === base || value.startsWith(`${base}/`);
}

export function localPathBasename(path: string): string {
  const normalized = normalizeLocalPath(path);
  if (normalized === '/') return '/';
  if (/^[A-Za-z]:\/$/.test(normalized)) return normalized.slice(0, 2);
  return normalized.split('/').filter(Boolean).at(-1) ?? normalized;
}

export function localPathParent(path: string): string {
  const normalized = normalizeLocalPath(path);
  if (!normalized || normalized === '/' || /^[A-Za-z]:\/$/.test(normalized)) return normalized;
  const index = normalized.lastIndexOf('/');
  const candidate = index === 0 ? '/' : normalized.slice(0, index);
  const parent = /^[A-Za-z]:$/.test(candidate) ? `${candidate}/` : candidate;
  return restoreLocalPathStyle(parent, path);
}

export function joinLocalPath(parent: string, name: string): string {
  const separator = localPathSeparator(parent);
  return `${parent.replace(/[\\/]$/, '')}${separator}${name}`;
}

export function siblingLocalPath(path: string, name: string): string {
  return joinLocalPath(localPathParent(path), name);
}

export function relativeLocalPath(path: string, root: string): string {
  const normalizedPath = normalizeLocalPath(path);
  const normalizedRoot = normalizeLocalPath(root);
  if (!localPathInside(normalizedRoot, normalizedPath)) return normalizedPath;
  if (sameLocalPath(normalizedPath, normalizedRoot)) return '';
  if (normalizedRoot === '/') return normalizedPath.slice(1);
  return normalizedPath.slice(normalizedRoot.length + 1);
}

export function rebaseLocalPath(path: string, source: string, destination: string): string {
  if (sameLocalPath(path, source)) return destination;
  if (!localPathInside(source, path)) return path;
  const relative = relativeLocalPath(path, source);
  return relative ? joinLocalPath(destination, relative) : destination;
}

export function canMoveLocalPaths(paths: readonly string[], destinationDirectory: string): boolean {
  const sources = uniqueLocalPaths(paths);
  if (!sources.length || !destinationDirectory) return false;
  if (
    sources.some(
      (source) => sameLocalPath(source, destinationDirectory) || localPathInside(source, destinationDirectory)
    )
  ) {
    return false;
  }
  return sources.some((source) => !sameLocalPath(localPathParent(source), destinationDirectory));
}

export function writeWorkLocalFileDragData(dataTransfer: DataTransfer, paths: readonly string[]): void {
  const sources = uniqueLocalPaths(paths).slice(0, 200);
  dataTransfer.effectAllowed = 'move';
  dataTransfer.setData(WORK_LOCAL_FILE_DRAG_TYPE, JSON.stringify(sources));
}

export function readWorkLocalFileDragData(dataTransfer: Pick<DataTransfer, 'getData'>): string[] {
  try {
    const value = JSON.parse(dataTransfer.getData(WORK_LOCAL_FILE_DRAG_TYPE)) as unknown;
    return Array.isArray(value)
      ? uniqueLocalPaths(value.filter((path): path is string => typeof path === 'string')).slice(0, 200)
      : [];
  } catch {
    return [];
  }
}

export function hasWorkLocalFileDragData(dataTransfer: Pick<DataTransfer, 'types'>): boolean {
  return Array.from(dataTransfer.types).includes(WORK_LOCAL_FILE_DRAG_TYPE);
}

export function workBreadcrumbs(root: string, current: string): WorkBreadcrumb[] {
  if (!root || !current || !localPathInside(root, current)) return [];
  const breadcrumbs: WorkBreadcrumb[] = [{ label: localPathBasename(root), path: root }];
  const relative = relativeLocalPath(current, root);
  if (!relative) return breadcrumbs;
  let path = root;
  for (const segment of relative.split('/').filter(Boolean)) {
    path = joinLocalPath(path, segment);
    breadcrumbs.push({ label: segment, path });
  }
  return breadcrumbs;
}

export function workFileExtension(path: string): string {
  const name = localPathBasename(path);
  const index = name.lastIndexOf('.');
  return index > 0 ? name.slice(index + 1).toLocaleLowerCase() : '';
}

export function workFileMimeType(path: string): string {
  const extension = workFileExtension(path);
  const mimeTypes: Record<string, string> = {
    docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    xls: 'application/vnd.ms-excel',
    ods: 'application/vnd.oasis.opendocument.spreadsheet',
    pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    pdf: 'application/pdf',
    csv: 'text/csv',
    html: 'text/html',
    htm: 'text/html',
    md: 'text/markdown',
    markdown: 'text/markdown',
    txt: 'text/plain',
    json: 'application/json',
    xml: 'application/xml',
    svg: 'image/svg+xml',
    png: 'image/png',
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    gif: 'image/gif',
    webp: 'image/webp',
    bmp: 'image/bmp',
    avif: 'image/avif',
  };
  return mimeTypes[extension] ?? 'application/octet-stream';
}

export function isWorkImportablePath(path: string): boolean {
  return importableExtensions.has(workFileExtension(path));
}

export function isWorkOfficePath(path: string): boolean {
  return officeEditorExtensions.has(workFileExtension(path));
}

export function isWorkTextEditorEntry(entry: Pick<WorkspaceEntry, 'isDirectory' | 'isBinary' | 'path'>): boolean {
  return !entry.isDirectory && !entry.isBinary && !isWorkOfficePath(entry.path);
}

export function isWorkOpenableEntry(entry: Pick<WorkspaceEntry, 'isDirectory' | 'isBinary' | 'path'>): boolean {
  return entry.isDirectory || isWorkOfficePath(entry.path) || isWorkTextEditorEntry(entry);
}

export function workFileKindLabel(entry: Pick<WorkspaceEntry, 'isDirectory' | 'path'>): string {
  if (entry.isDirectory) return '文件夹';
  const extension = workFileExtension(entry.path);
  if (['docx', 'doc'].includes(extension)) return '文字文档';
  if (['xlsx', 'xls', 'csv', 'ods'].includes(extension)) return '电子表格';
  if (['pptx', 'ppt'].includes(extension)) return '演示文稿';
  if (extension === 'pdf') return 'PDF 文档';
  if (['md', 'markdown', 'txt'].includes(extension)) return '文本';
  if (['html', 'htm'].includes(extension)) return '网页';
  if (
    [
      'c',
      'cc',
      'cpp',
      'cs',
      'css',
      'go',
      'h',
      'hpp',
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
    ].includes(extension)
  )
    return '代码';
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'heic'].includes(extension)) return '图像';
  if (['zip', '7z', 'rar', 'tar', 'gz'].includes(extension)) return '归档';
  return extension ? `${extension.toLocaleUpperCase()} 文件` : '文件';
}

export function formatWorkFileSize(size: number, directory = false): string {
  if (directory) return '—';
  if (!Number.isFinite(size) || size <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const exponent = Math.min(Math.floor(Math.log(size) / Math.log(1024)), units.length - 1);
  const value = size / 1024 ** exponent;
  const digits = exponent === 0 || value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[exponent]}`;
}

export function formatWorkFileDate(value?: number | null): string {
  if (!value || !Number.isFinite(value)) return '—';
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(value);
}

export function sortWorkFileEntries(entries: readonly WorkspaceEntry[], sort: WorkFilesSort): WorkspaceEntry[] {
  const direction = sort.direction === 'ascending' ? 1 : -1;
  return [...entries].sort((left, right) => {
    if (left.isDirectory !== right.isDirectory) return left.isDirectory ? -1 : 1;
    let comparison = 0;
    if (sort.key === 'modified') comparison = (left.mtimeMs ?? 0) - (right.mtimeMs ?? 0);
    else if (sort.key === 'size') comparison = left.size - right.size;
    else if (sort.key === 'kind') comparison = collator.compare(workFileKindLabel(left), workFileKindLabel(right));
    else comparison = collator.compare(left.name, right.name);
    return comparison === 0 ? collator.compare(left.name, right.name) : comparison * direction;
  });
}

export function workDuplicateName(name: string, directory: boolean): string {
  if (directory) return `${name} 副本`;
  const index = name.lastIndexOf('.');
  return index > 0 ? `${name.slice(0, index)} 副本${name.slice(index)}` : `${name} 副本`;
}

function localPathSeparator(path: string): '/' | '\\' {
  return path.includes('\\') && !path.includes('/') ? '\\' : '/';
}

function restoreLocalPathStyle(path: string, reference: string): string {
  return localPathSeparator(reference) === '\\' ? path.replace(/\//g, '\\') : path;
}

function uniqueLocalPaths(paths: readonly string[]): string[] {
  const result: string[] = [];
  for (const path of paths) {
    if (!path || result.some((candidate) => sameLocalPath(candidate, path))) continue;
    result.push(path);
  }
  return result;
}
