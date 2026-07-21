import { apiRequest } from '../../lib/api';
import type {
  WorkArtifact,
  WorkArtifactVersion,
  WorkFolder,
  WorkLibrarySnapshot,
  WorkStorageLimits,
} from './work-types';

interface WorkLibraryResponse {
  artifacts: WorkArtifact[];
  folders: WorkFolder[];
  limits: WorkStorageLimits;
  storage: 'server';
}

function revisionBody(expectedRevision: number): RequestInit {
  return {
    body: JSON.stringify({ expectedRevision }),
    headers: { 'Content-Type': 'application/json' },
  };
}

export const workApi = {
  library: (includeTrash = true) =>
    apiRequest<WorkLibraryResponse>(`/api/v1/work/library?includeTrash=${String(includeTrash)}`),
  artifact: (id: string) => apiRequest<WorkArtifact>(`/api/v1/work/artifacts/${encodeURIComponent(id)}`),
  saveArtifact: (artifact: WorkArtifact, expectedRevision: number) =>
    apiRequest<WorkArtifact>(`/api/v1/work/artifacts/${encodeURIComponent(artifact.id)}`, {
      method: 'PUT',
      body: JSON.stringify({ artifact, expectedRevision }),
      headers: { 'Content-Type': 'application/json' },
    }),
  trashArtifact: (id: string, expectedRevision: number) =>
    apiRequest<WorkArtifact>(`/api/v1/work/artifacts/${encodeURIComponent(id)}`, {
      method: 'DELETE',
      ...revisionBody(expectedRevision),
    }),
  restoreArtifact: (id: string, expectedRevision: number) =>
    apiRequest<WorkArtifact>(`/api/v1/work/artifacts/${encodeURIComponent(id)}/restore`, {
      method: 'POST',
      ...revisionBody(expectedRevision),
    }),
  purgeArtifact: (id: string) =>
    apiRequest<{ purged: boolean }>(`/api/v1/work/artifacts/${encodeURIComponent(id)}/purge`, {
      method: 'DELETE',
    }),
  copyArtifact: (id: string, copy: { id: string; title?: string; folderId?: string | null }) =>
    apiRequest<WorkArtifact>(`/api/v1/work/artifacts/${encodeURIComponent(id)}/copy`, {
      method: 'POST',
      body: JSON.stringify(copy),
      headers: { 'Content-Type': 'application/json' },
    }),
  versions: (id: string) =>
    apiRequest<WorkArtifactVersion[]>(`/api/v1/work/artifacts/${encodeURIComponent(id)}/versions`),
  restoreVersion: (id: string, version: number, expectedRevision: number) =>
    apiRequest<WorkArtifact>(`/api/v1/work/artifacts/${encodeURIComponent(id)}/versions/restore`, {
      method: 'POST',
      body: JSON.stringify({ version, expectedRevision }),
      headers: { 'Content-Type': 'application/json' },
    }),
  uploadSource: (id: string, expectedRevision: number, file: File) =>
    apiRequest<WorkArtifact>(
      `/api/v1/work/artifacts/${encodeURIComponent(id)}/source?expectedRevision=${expectedRevision}&fileName=${encodeURIComponent(file.name)}`,
      {
        method: 'PUT',
        body: file,
        headers: { 'Content-Type': file.type || 'application/octet-stream' },
      }
    ),
  sourceUrl: (id: string) => `/api/v1/work/artifacts/${encodeURIComponent(id)}/source`,
  saveFolder: (folder: WorkFolder, expectedRevision: number) =>
    apiRequest<WorkFolder>(`/api/v1/work/folders/${encodeURIComponent(folder.id)}`, {
      method: 'PUT',
      body: JSON.stringify({ folder, expectedRevision }),
      headers: { 'Content-Type': 'application/json' },
    }),
  trashFolder: (id: string, expectedRevision: number) =>
    apiRequest<WorkFolder>(`/api/v1/work/folders/${encodeURIComponent(id)}`, {
      method: 'DELETE',
      ...revisionBody(expectedRevision),
    }),
  restoreFolder: (id: string, expectedRevision: number) =>
    apiRequest<WorkFolder>(`/api/v1/work/folders/${encodeURIComponent(id)}/restore`, {
      method: 'POST',
      ...revisionBody(expectedRevision),
    }),
  purgeFolder: (id: string) =>
    apiRequest<{ purged: boolean }>(`/api/v1/work/folders/${encodeURIComponent(id)}/purge`, {
      method: 'DELETE',
    }),
} satisfies Record<string, unknown>;

export function serverLibrarySnapshot(response: WorkLibraryResponse): WorkLibrarySnapshot {
  return {
    artifacts: response.artifacts,
    folders: response.folders,
    limits: response.limits,
    storage: 'server',
  };
}
