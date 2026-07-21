import type { WorkspaceEntry } from '../../types/api';
import { localPathInside, normalizeLocalPath } from './work-local-files';

export interface WorkLocalFileSearchOptions {
  maxDirectories?: number;
  maxEntries?: number;
  maxResults?: number;
  concurrency?: number;
}

export interface WorkLocalFileSearchResult {
  entries: WorkspaceEntry[];
  scannedDirectories: number;
  scannedEntries: number;
  unreadableDirectories: number;
  truncated: boolean;
}

export type WorkLocalDirectoryReader = (path: string) => Promise<WorkspaceEntry[]>;

const defaultMaxDirectories = 400;
const defaultMaxEntries = 10_000;
const defaultMaxResults = 250;
const defaultConcurrency = 6;

export async function searchWorkLocalFiles(
  readDirectory: WorkLocalDirectoryReader,
  rootPath: string,
  query: string,
  options: WorkLocalFileSearchOptions = {}
): Promise<WorkLocalFileSearchResult> {
  const root = rootPath.trim();
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!root || !normalizedQuery) return emptySearchResult();

  const maxDirectories = positiveLimit(options.maxDirectories, defaultMaxDirectories);
  const maxEntries = positiveLimit(options.maxEntries, defaultMaxEntries);
  const maxResults = positiveLimit(options.maxResults, defaultMaxResults);
  const concurrency = Math.min(12, positiveLimit(options.concurrency, defaultConcurrency));
  const windowsRoot = /^[A-Za-z]:[\\/]/.test(root);
  const pathKey = (path: string) => {
    const normalized = normalizeLocalPath(path);
    return windowsRoot ? normalized.toLocaleLowerCase() : normalized;
  };
  const queue = [root];
  const visited = new Set([pathKey(root)]);
  const entries: WorkspaceEntry[] = [];
  let scannedDirectories = 0;
  let scannedEntries = 0;
  let unreadableDirectories = 0;
  let truncated = false;

  while (
    queue.length &&
    scannedDirectories < maxDirectories &&
    scannedEntries < maxEntries &&
    entries.length < maxResults
  ) {
    const batchSize = Math.min(concurrency, queue.length, maxDirectories - scannedDirectories);
    const paths = queue.splice(0, batchSize);
    scannedDirectories += paths.length;
    const listings = await Promise.all(
      paths.map(async (path) => {
        try {
          return { path, entries: await readDirectory(path), error: null };
        } catch (error) {
          return { path, entries: [] as WorkspaceEntry[], error };
        }
      })
    );

    for (const listing of listings) {
      if (listing.error) {
        if (pathKey(listing.path) === pathKey(root)) throw listing.error;
        unreadableDirectories += 1;
        truncated = true;
        continue;
      }
      for (const entry of listing.entries) {
        if (scannedEntries >= maxEntries || entries.length >= maxResults) {
          truncated = true;
          break;
        }
        if (!localPathInside(root, entry.path)) continue;
        scannedEntries += 1;
        if (entry.name.toLocaleLowerCase().includes(normalizedQuery)) entries.push(entry);
        if (entry.isDirectory) {
          const key = pathKey(entry.path);
          if (!visited.has(key)) {
            visited.add(key);
            queue.push(entry.path);
          }
        }
      }
    }
  }

  if (
    queue.length ||
    scannedDirectories >= maxDirectories ||
    scannedEntries >= maxEntries ||
    entries.length >= maxResults
  ) {
    truncated = true;
  }
  return { entries, scannedDirectories, scannedEntries, unreadableDirectories, truncated };
}

function positiveLimit(value: number | undefined, fallback: number): number {
  return Number.isInteger(value) && Number(value) > 0 ? Number(value) : fallback;
}

function emptySearchResult(): WorkLocalFileSearchResult {
  return {
    entries: [],
    scannedDirectories: 0,
    scannedEntries: 0,
    unreadableDirectories: 0,
    truncated: false,
  };
}
