import type { WorkArtifact, WorkFolder, WorkLibrarySnapshot } from './work-types';

const DATABASE_NAME = 'a3s-work';
const DATABASE_VERSION = 3;
const ARTIFACT_STORE = 'artifacts';
const FOLDER_STORE = 'folders';
const SOURCE_STORE = 'sources';
const FALLBACK_ARTIFACT_KEY = 'a3s-work.artifacts.v1';
const FALLBACK_FOLDER_KEY = 'a3s-work.folders.v1';

let databasePromise: Promise<IDBDatabase> | null = null;
const memorySources = new Map<string, Blob>();

export async function readLocalWorkLibrary(): Promise<WorkLibrarySnapshot> {
  return {
    artifacts: await readAllLocalArtifacts(),
    folders: await readAllLocalFolders(),
    limits: null,
    storage: 'local',
  };
}

export async function replaceLocalWorkLibrary(library: WorkLibrarySnapshot): Promise<void> {
  if (canUseIndexedDb()) {
    try {
      await replaceIndexedDbStore(ARTIFACT_STORE, library.artifacts);
      await replaceIndexedDbStore(FOLDER_STORE, library.folders);
      return;
    } catch {
      databasePromise = null;
    }
  }
  localStorage.setItem(FALLBACK_ARTIFACT_KEY, JSON.stringify(library.artifacts));
  localStorage.setItem(FALLBACK_FOLDER_KEY, JSON.stringify(library.folders));
}

export async function saveLocalWorkArtifact(artifact: WorkArtifact): Promise<void> {
  const snapshot = cloneArtifact(artifact);
  if (canUseIndexedDb()) {
    try {
      await withStore('readwrite', ARTIFACT_STORE, (store) => store.put(snapshot));
      return;
    } catch {
      databasePromise = null;
    }
  }
  const artifacts = readFallbackArtifacts();
  const index = artifacts.findIndex((item) => item.id === snapshot.id);
  if (index >= 0) artifacts[index] = snapshot;
  else artifacts.push(snapshot);
  localStorage.setItem(FALLBACK_ARTIFACT_KEY, JSON.stringify(artifacts));
}

export async function saveLocalWorkFolder(folder: WorkFolder): Promise<void> {
  const snapshot = cloneFolder(folder);
  if (canUseIndexedDb()) {
    try {
      await withStore('readwrite', FOLDER_STORE, (store) => store.put(snapshot));
      return;
    } catch {
      databasePromise = null;
    }
  }
  const folders = readFallbackFolders();
  const index = folders.findIndex((item) => item.id === snapshot.id);
  if (index >= 0) folders[index] = snapshot;
  else folders.push(snapshot);
  localStorage.setItem(FALLBACK_FOLDER_KEY, JSON.stringify(folders));
}

export async function removeLocalWorkArtifact(id: string): Promise<void> {
  if (canUseIndexedDb()) {
    try {
      await withStore('readwrite', ARTIFACT_STORE, (store) => store.delete(id));
      await withStore('readwrite', SOURCE_STORE, (store) => store.delete(id));
      return;
    } catch {
      databasePromise = null;
    }
  }
  localStorage.setItem(
    FALLBACK_ARTIFACT_KEY,
    JSON.stringify(readFallbackArtifacts().filter((artifact) => artifact.id !== id))
  );
  memorySources.delete(id);
}

export async function removeLocalWorkFolder(id: string): Promise<void> {
  if (canUseIndexedDb()) {
    try {
      await withStore('readwrite', FOLDER_STORE, (store) => store.delete(id));
      return;
    } catch {
      databasePromise = null;
    }
  }
  localStorage.setItem(FALLBACK_FOLDER_KEY, JSON.stringify(readFallbackFolders().filter((folder) => folder.id !== id)));
}

export async function readLocalWorkArtifact(id: string): Promise<WorkArtifact | null> {
  if (canUseIndexedDb()) {
    try {
      return (await withStore<WorkArtifact | undefined>('readonly', ARTIFACT_STORE, (store) => store.get(id))) ?? null;
    } catch {
      databasePromise = null;
    }
  }
  return readFallbackArtifacts().find((artifact) => artifact.id === id) ?? null;
}

export async function saveLocalWorkSource(id: string, source: Blob): Promise<void> {
  if (canUseIndexedDb()) {
    try {
      await withStore('readwrite', SOURCE_STORE, (store) => store.put({ id, source }));
      return;
    } catch {
      databasePromise = null;
    }
  }
  memorySources.set(id, source);
}

export async function readLocalWorkSource(id: string): Promise<Blob | null> {
  if (canUseIndexedDb()) {
    try {
      const record = await withStore<{ id: string; source: Blob } | undefined>('readonly', SOURCE_STORE, (store) =>
        store.get(id)
      );
      return record?.source ?? null;
    } catch {
      databasePromise = null;
    }
  }
  return memorySources.get(id) ?? null;
}

async function readAllLocalArtifacts(): Promise<WorkArtifact[]> {
  if (canUseIndexedDb()) {
    try {
      return await withStore<WorkArtifact[]>('readonly', ARTIFACT_STORE, (store) => store.getAll());
    } catch {
      databasePromise = null;
    }
  }
  return readFallbackArtifacts();
}

async function readAllLocalFolders(): Promise<WorkFolder[]> {
  if (canUseIndexedDb()) {
    try {
      return await withStore<WorkFolder[]>('readonly', FOLDER_STORE, (store) => store.getAll());
    } catch {
      databasePromise = null;
    }
  }
  return readFallbackFolders();
}

function readFallbackArtifacts(): WorkArtifact[] {
  return readFallbackArray(FALLBACK_ARTIFACT_KEY, isWorkArtifact).map(cloneArtifact);
}

function readFallbackFolders(): WorkFolder[] {
  return readFallbackArray(FALLBACK_FOLDER_KEY, isWorkFolder).map(cloneFolder);
}

function readFallbackArray<T>(key: string, validate: (value: unknown) => value is T): T[] {
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? '[]') as unknown;
    if (!Array.isArray(value)) return [];
    return value.filter(validate);
  } catch {
    return [];
  }
}

function canUseIndexedDb(): boolean {
  return typeof indexedDB !== 'undefined';
}

function openDatabase(): Promise<IDBDatabase> {
  if (databasePromise) return databasePromise;
  databasePromise = new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.onerror = () => reject(request.error ?? new Error('Unable to open the Work document database'));
    request.onblocked = () => reject(new Error('The Work document database is blocked by another tab'));
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(ARTIFACT_STORE)) {
        const store = database.createObjectStore(ARTIFACT_STORE, { keyPath: 'id' });
        store.createIndex('lastOpenedAt', 'lastOpenedAt');
        store.createIndex('kind', 'kind');
      }
      if (!database.objectStoreNames.contains(FOLDER_STORE)) {
        database.createObjectStore(FOLDER_STORE, { keyPath: 'id' });
      }
      if (!database.objectStoreNames.contains(SOURCE_STORE)) {
        database.createObjectStore(SOURCE_STORE, { keyPath: 'id' });
      }
    };
    request.onsuccess = () => {
      const database = request.result;
      database.onversionchange = () => {
        database.close();
        databasePromise = null;
      };
      resolve(database);
    };
  });
  return databasePromise;
}

async function withStore<T = IDBValidKey>(
  mode: IDBTransactionMode,
  storeName: string,
  operation: (store: IDBObjectStore) => IDBRequest<T>
): Promise<T> {
  const database = await openDatabase();
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(storeName, mode);
    const request = operation(transaction.objectStore(storeName));
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('Work document storage failed'));
    transaction.onabort = () => reject(transaction.error ?? new Error('Work document transaction was aborted'));
  });
}

async function replaceIndexedDbStore(storeName: string, values: Array<WorkArtifact | WorkFolder>): Promise<void> {
  const database = await openDatabase();
  await new Promise<void>((resolve, reject) => {
    const transaction = database.transaction(storeName, 'readwrite');
    const store = transaction.objectStore(storeName);
    store.clear();
    for (const value of values) store.put(value);
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error('Work document storage failed'));
    transaction.onabort = () => reject(transaction.error ?? new Error('Work document transaction was aborted'));
  });
}

function isWorkArtifact(value: unknown): value is WorkArtifact {
  if (!value || typeof value !== 'object') return false;
  const artifact = value as Partial<WorkArtifact>;
  return (
    typeof artifact.id === 'string' &&
    typeof artifact.title === 'string' &&
    (artifact.kind === 'document' ||
      artifact.kind === 'spreadsheet' ||
      artifact.kind === 'presentation' ||
      artifact.kind === 'pdf') &&
    typeof artifact.content === 'object' &&
    artifact.content !== null
  );
}

function isWorkFolder(value: unknown): value is WorkFolder {
  if (!value || typeof value !== 'object') return false;
  const folder = value as Partial<WorkFolder>;
  return typeof folder.id === 'string' && typeof folder.name === 'string' && typeof folder.revision === 'number';
}

function cloneArtifact(artifact: WorkArtifact): WorkArtifact {
  if (typeof structuredClone === 'function') return structuredClone(artifact);
  return JSON.parse(JSON.stringify(artifact)) as WorkArtifact;
}

function cloneFolder(folder: WorkFolder): WorkFolder {
  if (typeof structuredClone === 'function') return structuredClone(folder);
  return JSON.parse(JSON.stringify(folder)) as WorkFolder;
}
