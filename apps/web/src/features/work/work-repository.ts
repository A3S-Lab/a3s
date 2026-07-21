import { ApiError } from '../../lib/api';
import { serverLibrarySnapshot, workApi } from './work-api';
import {
  readLocalWorkArtifact,
  readLocalWorkLibrary,
  readLocalWorkSource,
  removeLocalWorkArtifact,
  removeLocalWorkFolder,
  replaceLocalWorkLibrary,
  saveLocalWorkArtifact,
  saveLocalWorkFolder,
  saveLocalWorkSource,
} from './work-local-cache';
import {
  clearArtifactPending,
  clearFolderPending,
  markArtifactPending,
  markArtifactPurgePending,
  markFolderPending,
  markFolderPurgePending,
  readWorkSyncState,
  updateWorkSyncState,
  type WorkSyncState,
} from './work-sync-state';
import { createWorkId } from './work-templates';
import type { WorkArtifact, WorkArtifactVersion, WorkFolder, WorkLibrarySnapshot, WorkStorageMode } from './work-types';

let storageMode: WorkStorageMode | null = null;
const serverArtifactRevisions = new Map<string, number>();
const serverFolderRevisions = new Map<string, number>();

export async function loadWorkLibrary(): Promise<WorkLibrarySnapshot> {
  const local = await readLocalWorkLibrary();
  try {
    let server = serverLibrarySnapshot(await workApi.library(true));
    storageMode = 'server';
    rememberServerRevisions(server);
    server = await reconcileLocalLibrary(local, server);
    await replaceLocalWorkLibrary(server);
    return sortLibrary(server);
  } catch (error) {
    if (!isUnavailableWorkApi(error)) throw error;
    storageMode = 'local';
    return sortLibrary(local);
  }
}

export async function listWorkArtifacts(): Promise<WorkArtifact[]> {
  return (await loadWorkLibrary()).artifacts.filter((artifact) => !artifact.trashedAt);
}

export async function saveWorkArtifact(artifact: WorkArtifact): Promise<WorkArtifact> {
  const snapshot = cloneArtifact(artifact);
  if (storageMode !== 'local') {
    try {
      const saved = await workApi.saveArtifact(snapshot, serverArtifactRevisions.get(snapshot.id) ?? 0);
      storageMode = 'server';
      serverArtifactRevisions.set(saved.id, saved.revision);
      await saveLocalWorkArtifact(saved);
      clearArtifactPending(saved.id);
      return cloneArtifact(saved);
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  await saveLocalWorkArtifact(snapshot);
  markArtifactPending(snapshot.id);
  return snapshot;
}

export async function deleteWorkArtifact(id: string): Promise<WorkArtifact | null> {
  const local = await readLocalWorkArtifact(id);
  if (storageMode !== 'local') {
    try {
      const trashed = await workApi.trashArtifact(id, serverArtifactRevisions.get(id) ?? local?.revision ?? 0);
      storageMode = 'server';
      serverArtifactRevisions.set(id, trashed.revision);
      await saveLocalWorkArtifact(trashed);
      clearArtifactPending(id);
      return cloneArtifact(trashed);
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  if (!local) return null;
  const trashed = {
    ...local,
    trashedAt: Date.now(),
    updatedAt: Date.now(),
    revision: local.revision + 1,
  };
  await saveLocalWorkArtifact(trashed);
  markArtifactPending(id);
  return trashed;
}

export async function restoreWorkArtifact(artifact: WorkArtifact): Promise<WorkArtifact> {
  if (storageMode !== 'local') {
    try {
      const restored = await workApi.restoreArtifact(
        artifact.id,
        serverArtifactRevisions.get(artifact.id) ?? artifact.revision
      );
      storageMode = 'server';
      serverArtifactRevisions.set(restored.id, restored.revision);
      await saveLocalWorkArtifact(restored);
      clearArtifactPending(restored.id);
      return cloneArtifact(restored);
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  const restored = {
    ...cloneArtifact(artifact),
    trashedAt: null,
    updatedAt: Date.now(),
    revision: artifact.revision + 1,
  };
  await saveLocalWorkArtifact(restored);
  markArtifactPending(restored.id);
  return restored;
}

export async function purgeWorkArtifact(id: string): Promise<void> {
  if (storageMode !== 'local') {
    try {
      await workApi.purgeArtifact(id);
      storageMode = 'server';
      serverArtifactRevisions.delete(id);
      await removeLocalWorkArtifact(id);
      clearArtifactPending(id);
      return;
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  await removeLocalWorkArtifact(id);
  markArtifactPurgePending(id);
}

export async function copyWorkArtifact(
  artifact: WorkArtifact,
  folderId: string | null = artifact.folderId ?? null
): Promise<WorkArtifact> {
  const id = createWorkId('artifact');
  const title = `${artifact.title} 副本`;
  if (storageMode !== 'local') {
    try {
      const copy = await workApi.copyArtifact(artifact.id, { id, title, folderId });
      storageMode = 'server';
      serverArtifactRevisions.set(copy.id, copy.revision);
      await saveLocalWorkArtifact(copy);
      clearArtifactPending(copy.id);
      return cloneArtifact(copy);
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  const now = Date.now();
  const copy: WorkArtifact = {
    ...cloneArtifact(artifact),
    id,
    title,
    folderId,
    source: null,
    trashedAt: null,
    createdAt: now,
    updatedAt: now,
    lastOpenedAt: now,
    revision: 1,
  };
  await saveLocalWorkArtifact(copy);
  markArtifactPending(copy.id);
  return copy;
}

export async function saveWorkSource(artifact: WorkArtifact, file: File): Promise<WorkArtifact> {
  await saveLocalWorkSource(artifact.id, file);
  if (storageMode !== 'server') {
    const saved = {
      ...cloneArtifact(artifact),
      revision: artifact.revision + 1,
      updatedAt: Date.now(),
      source: {
        name: file.name,
        contentType: file.type || 'application/octet-stream',
        size: file.size,
        updatedAt: Date.now(),
      },
    };
    await saveLocalWorkArtifact(saved);
    markArtifactPending(saved.id);
    return saved;
  }
  const saved = await workApi.uploadSource(
    artifact.id,
    serverArtifactRevisions.get(artifact.id) ?? artifact.revision,
    file
  );
  serverArtifactRevisions.set(saved.id, saved.revision);
  await saveLocalWorkArtifact(saved);
  clearArtifactPending(saved.id);
  return cloneArtifact(saved);
}

export async function downloadWorkSource(artifact: WorkArtifact): Promise<void> {
  if (!artifact.source) throw new Error('此文件没有可下载的原始文件');
  downloadBlob(await readWorkSourceBlob(artifact), artifact.source.name);
}

export async function readWorkSourceBlob(artifact: WorkArtifact): Promise<Blob> {
  if (!artifact.source) throw new Error('此文件没有可读取的原始文件');
  const cached = await readLocalWorkSource(artifact.id);
  if (cached) return cached;
  if (storageMode !== 'server') throw new Error('原始文件不在此设备的兼容缓存中');
  const response = await fetch(workApi.sourceUrl(artifact.id), { headers: { Accept: '*/*' } });
  if (!response.ok) throw new Error(`原始文件读取失败（HTTP ${response.status}）`);
  const blob = await response.blob();
  await saveLocalWorkSource(artifact.id, blob);
  return blob;
}

export async function listWorkArtifactVersions(artifact: WorkArtifact): Promise<WorkArtifactVersion[]> {
  if (storageMode !== 'server') {
    return [{ revision: artifact.revision, updatedAt: artifact.updatedAt, current: true, artifact }];
  }
  return workApi.versions(artifact.id);
}

export async function restoreWorkArtifactVersion(artifact: WorkArtifact, version: number): Promise<WorkArtifact> {
  if (storageMode !== 'server') return artifact;
  const restored = await workApi.restoreVersion(
    artifact.id,
    version,
    serverArtifactRevisions.get(artifact.id) ?? artifact.revision
  );
  serverArtifactRevisions.set(restored.id, restored.revision);
  await saveLocalWorkArtifact(restored);
  clearArtifactPending(restored.id);
  return cloneArtifact(restored);
}

export async function saveWorkFolder(folder: WorkFolder): Promise<WorkFolder> {
  const snapshot = cloneFolder(folder);
  if (storageMode !== 'local') {
    try {
      const saved = await workApi.saveFolder(snapshot, serverFolderRevisions.get(snapshot.id) ?? 0);
      storageMode = 'server';
      serverFolderRevisions.set(saved.id, saved.revision);
      await saveLocalWorkFolder(saved);
      clearFolderPending(saved.id);
      return cloneFolder(saved);
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  await saveLocalWorkFolder(snapshot);
  markFolderPending(snapshot.id);
  return snapshot;
}

export async function trashWorkFolder(folder: WorkFolder): Promise<WorkFolder> {
  if (storageMode !== 'local') {
    try {
      const trashed = await workApi.trashFolder(folder.id, serverFolderRevisions.get(folder.id) ?? folder.revision);
      storageMode = 'server';
      serverFolderRevisions.set(trashed.id, trashed.revision);
      await saveLocalWorkFolder(trashed);
      clearFolderPending(trashed.id);
      return cloneFolder(trashed);
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  const trashed = {
    ...cloneFolder(folder),
    trashedAt: Date.now(),
    updatedAt: Date.now(),
    revision: folder.revision + 1,
  };
  await saveLocalWorkFolder(trashed);
  markFolderPending(trashed.id);
  return trashed;
}

export async function restoreWorkFolder(folder: WorkFolder): Promise<WorkFolder> {
  if (storageMode !== 'local') {
    try {
      const restored = await workApi.restoreFolder(folder.id, serverFolderRevisions.get(folder.id) ?? folder.revision);
      storageMode = 'server';
      serverFolderRevisions.set(restored.id, restored.revision);
      await saveLocalWorkFolder(restored);
      clearFolderPending(restored.id);
      return cloneFolder(restored);
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  const restored = {
    ...cloneFolder(folder),
    trashedAt: null,
    updatedAt: Date.now(),
    revision: folder.revision + 1,
  };
  await saveLocalWorkFolder(restored);
  markFolderPending(restored.id);
  return restored;
}

export async function purgeWorkFolder(id: string): Promise<void> {
  if (storageMode !== 'local') {
    try {
      await workApi.purgeFolder(id);
      storageMode = 'server';
      serverFolderRevisions.delete(id);
      await removeLocalWorkFolder(id);
      clearFolderPending(id);
      return;
    } catch (error) {
      if (!isUnavailableWorkApi(error)) throw error;
      storageMode = 'local';
    }
  }
  await removeLocalWorkFolder(id);
  markFolderPurgePending(id);
}

export function workRepositoryStorageMode(): WorkStorageMode {
  return storageMode ?? 'local';
}

function rememberServerRevisions(library: WorkLibrarySnapshot): void {
  serverArtifactRevisions.clear();
  serverFolderRevisions.clear();
  for (const artifact of library.artifacts) serverArtifactRevisions.set(artifact.id, artifact.revision);
  for (const folder of library.folders) serverFolderRevisions.set(folder.id, folder.revision);
}

async function reconcileLocalLibrary(
  local: WorkLibrarySnapshot,
  server: WorkLibrarySnapshot
): Promise<WorkLibrarySnapshot> {
  const sync = readWorkSyncState();
  await reconcileArtifactPurges(server, sync);
  await reconcileFolderPurges(server, sync);
  for (const folder of parentFirstFolders(local.folders)) {
    const existing = server.folders.find((item) => item.id === folder.id);
    const pending = sync.folders.includes(folder.id);
    if (existing && !pending) continue;
    if (!pending && sync.migrated) continue;
    const saved = await workApi.saveFolder(folder, existing?.revision ?? 0);
    replaceById(server.folders, saved);
    serverFolderRevisions.set(saved.id, saved.revision);
    clearFolderPending(saved.id);
  }
  for (const artifact of local.artifacts) {
    const existing = server.artifacts.find((item) => item.id === artifact.id);
    const pending = sync.artifacts.includes(artifact.id);
    if (existing && !pending) continue;
    if (!pending && sync.migrated) continue;
    let saved = await workApi.saveArtifact(artifact, existing?.revision ?? 0);
    const source = artifact.source ? await readLocalWorkSource(artifact.id) : null;
    if (source && artifact.source) {
      saved = await workApi.uploadSource(
        saved.id,
        saved.revision,
        new File([source], artifact.source.name, { type: artifact.source.contentType })
      );
    }
    replaceById(server.artifacts, saved);
    serverArtifactRevisions.set(saved.id, saved.revision);
    clearArtifactPending(saved.id);
  }
  updateWorkSyncState((state) => ({ ...state, migrated: true }));
  return server;
}

async function reconcileArtifactPurges(server: WorkLibrarySnapshot, sync: WorkSyncState): Promise<void> {
  for (const id of sync.artifactPurges) {
    let artifact = server.artifacts.find((item) => item.id === id);
    if (!artifact) {
      clearArtifactPending(id);
      continue;
    }
    if (!artifact.trashedAt) {
      artifact = await workApi.trashArtifact(id, artifact.revision);
      serverArtifactRevisions.set(id, artifact.revision);
    }
    await workApi.purgeArtifact(id);
    server.artifacts = server.artifacts.filter((item) => item.id !== id);
    serverArtifactRevisions.delete(id);
    clearArtifactPending(id);
  }
}

async function reconcileFolderPurges(server: WorkLibrarySnapshot, sync: WorkSyncState): Promise<void> {
  for (const id of sync.folderPurges) {
    let folder = server.folders.find((item) => item.id === id);
    if (!folder) {
      clearFolderPending(id);
      continue;
    }
    if (!folder.trashedAt) {
      folder = await workApi.trashFolder(id, folder.revision);
      serverFolderRevisions.set(id, folder.revision);
    }
    await workApi.purgeFolder(id);
    server.folders = server.folders.filter((item) => item.id !== id);
    serverFolderRevisions.delete(id);
    clearFolderPending(id);
  }
}

function replaceById<T extends { id: string }>(items: T[], replacement: T): void {
  const index = items.findIndex((item) => item.id === replacement.id);
  if (index >= 0) items[index] = replacement;
  else items.push(replacement);
}

function parentFirstFolders(folders: WorkFolder[]): WorkFolder[] {
  const byId = new Map(folders.map((folder) => [folder.id, folder]));
  const depth = (folder: WorkFolder, visited = new Set<string>()): number => {
    if (!folder.parentId || visited.has(folder.id)) return 0;
    const parent = byId.get(folder.parentId);
    if (!parent) return 0;
    visited.add(folder.id);
    return 1 + depth(parent, visited);
  };
  return [...folders].sort((left, right) => depth(left) - depth(right));
}

function isUnavailableWorkApi(error: unknown): boolean {
  // Once the server has owned this session, a failed request must surface as a
  // failed save. Falling back silently would make "saved" mean only cached.
  if (storageMode === 'server') return false;
  return (
    (error instanceof ApiError && (error.status === 404 || error.status === 405)) ||
    error instanceof TypeError ||
    error instanceof ReferenceError
  );
}

function sortLibrary(library: WorkLibrarySnapshot): WorkLibrarySnapshot {
  return {
    ...library,
    artifacts: [...library.artifacts].sort(
      (left, right) => right.lastOpenedAt - left.lastOpenedAt || right.updatedAt - left.updatedAt
    ),
    folders: [...library.folders].sort((left, right) => left.name.localeCompare(right.name, 'zh-CN')),
  };
}

function cloneArtifact(artifact: WorkArtifact): WorkArtifact {
  if (typeof structuredClone === 'function') return structuredClone(artifact);
  return JSON.parse(JSON.stringify(artifact)) as WorkArtifact;
}

function cloneFolder(folder: WorkFolder): WorkFolder {
  if (typeof structuredClone === 'function') return structuredClone(folder);
  return JSON.parse(JSON.stringify(folder)) as WorkFolder;
}

function downloadBlob(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}
