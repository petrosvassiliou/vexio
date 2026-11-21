/**
 * Dedicated Origin Private File System (OPFS) Persistence Worker.
 *
 * Uses synchronous `FileSystemSyncAccessHandle` (`createSyncAccessHandle()`) inside
 * this dedicated Web Worker for zero-latency offline swap files, CRC32 WAL undo/redo
 * journals, crash-proof autosave recovery, immutable Version History snapshots,
 * multi-document local workspaces, and cached custom `.vssx` stencil packs.
 */

/// <reference lib="webworker" />

interface FileSystemSyncAccessHandle {
  read(buffer: ArrayBuffer | ArrayBufferView, options?: { at?: number }): number;
  write(buffer: ArrayBuffer | ArrayBufferView, options?: { at?: number }): number;
  truncate(newSize: number): void;
  getSize(): number;
  flush(): void;
  close(): void;
}

interface SyncCapableFileHandle extends FileSystemFileHandle {
  createSyncAccessHandle(): Promise<FileSystemSyncAccessHandle>;
}

export interface OpfsVersionMeta {
  versionId: string;
  docId: string;
  label: string;
  byteLength: number;
  createdAt: string;
}

export interface OpfsWorkspaceMeta {
  docId: string;
  title: string;
  byteLength: number;
  updatedAt: string;
}

type OpfsFolder = 'wal' | 'autosave' | 'stencils' | 'versions' | 'workspaces';

export type OpfsWorkerRequest =
  | { id: number; op: 'writeWal'; docId: string; bytes: Uint8Array }
  | { id: number; op: 'readWal'; docId: string }
  | { id: number; op: 'writeAutosave'; docId: string; vsdxBytes: Uint8Array }
  | { id: number; op: 'readAutosave'; docId: string }
  | { id: number; op: 'cacheStencil'; name: string; vssxBytes: Uint8Array }
  | { id: number; op: 'listStencils' }
  | { id: number; op: 'saveVersion'; docId: string; versionId: string; label: string; vsdxBytes: Uint8Array }
  | { id: number; op: 'listVersions'; docId: string }
  | { id: number; op: 'loadVersion'; docId: string; versionId: string }
  | { id: number; op: 'saveWorkspace'; docId: string; title: string; vsdxBytes: Uint8Array }
  | { id: number; op: 'listWorkspaces' }
  | { id: number; op: 'loadWorkspace'; docId: string };

export type OpfsWorkerResponse =
  | {
      id: number;
      ok: true;
      bytes?: Uint8Array;
      stencils?: string[];
      versions?: OpfsVersionMeta[];
      workspaces?: OpfsWorkspaceMeta[];
      byteLength?: number;
    }
  | { id: number; ok: false; error: string };

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

async function getSubDir(name: OpfsFolder): Promise<FileSystemDirectoryHandle> {
  const root = await navigator.storage.getDirectory();
  return root.getDirectoryHandle(name, { create: true });
}

async function withSyncHandle<R>(
  folder: OpfsFolder,
  fileName: string,
  fn: (handle: FileSystemSyncAccessHandle) => R
): Promise<R> {
  const safeName = fileName.replace(/[^a-zA-Z0-9._-]/g, '_');
  const dir = await getSubDir(folder);
  const fileHandle = (await dir.getFileHandle(safeName, { create: true })) as SyncCapableFileHandle;
  const syncHandle = await fileHandle.createSyncAccessHandle();
  try {
    return fn(syncHandle);
  } finally {
    try {
      syncHandle.close();
    } catch {
      // Ignore close error
    }
  }
}

function writeSyncAtomic(handle: FileSystemSyncAccessHandle, data: Uint8Array): number {
  handle.truncate(0);
  const written = handle.write(data, { at: 0 });
  handle.flush();
  return written;
}

function readSyncAll(handle: FileSystemSyncAccessHandle): Uint8Array {
  const size = handle.getSize();
  const out = new Uint8Array(size);
  if (size > 0) {
    handle.read(out, { at: 0 });
  }
  return out;
}

async function readJsonIndex<T>(folder: OpfsFolder, fileName: string): Promise<T[]> {
  const bytes = await withSyncHandle(folder, fileName, (h) => readSyncAll(h));
  if (bytes.byteLength === 0) return [];
  try {
    const parsed = JSON.parse(textDecoder.decode(bytes));
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

async function writeJsonIndex<T>(folder: OpfsFolder, fileName: string, items: T[]): Promise<void> {
  await withSyncHandle(folder, fileName, (h) =>
    writeSyncAtomic(h, textEncoder.encode(JSON.stringify(items)))
  );
}

self.onmessage = async (ev: MessageEvent<OpfsWorkerRequest>) => {
  const req = ev.data;
  try {
    switch (req.op) {
      case 'writeWal': {
        const byteLength = await withSyncHandle('wal', `${req.docId}.wal`, (h) =>
          writeSyncAtomic(h, req.bytes)
        );
        self.postMessage({ id: req.id, ok: true, byteLength } satisfies OpfsWorkerResponse);
        break;
      }
      case 'readWal': {
        const bytes = await withSyncHandle('wal', `${req.docId}.wal`, (h) => readSyncAll(h));
        self.postMessage(
          { id: req.id, ok: true, bytes } satisfies OpfsWorkerResponse,
          { transfer: [bytes.buffer] }
        );
        break;
      }
      case 'writeAutosave': {
        const byteLength = await withSyncHandle('autosave', `${req.docId}.vsdx`, (h) =>
          writeSyncAtomic(h, req.vsdxBytes)
        );
        self.postMessage({ id: req.id, ok: true, byteLength } satisfies OpfsWorkerResponse);
        break;
      }
      case 'readAutosave': {
        const bytes = await withSyncHandle('autosave', `${req.docId}.vsdx`, (h) => readSyncAll(h));
        self.postMessage(
          { id: req.id, ok: true, bytes } satisfies OpfsWorkerResponse,
          { transfer: [bytes.buffer] }
        );
        break;
      }
      case 'cacheStencil': {
        const byteLength = await withSyncHandle('stencils', req.name, (h) =>
          writeSyncAtomic(h, req.vssxBytes)
        );
        self.postMessage({ id: req.id, ok: true, byteLength } satisfies OpfsWorkerResponse);
        break;
      }
      case 'listStencils': {
        const dir = await getSubDir('stencils');
        const names: string[] = [];
        // @ts-expect-error AsyncIterator on FileSystemDirectoryHandle in ChromeOS
        for await (const [entryName] of dir.entries()) {
          names.push(entryName);
        }
        self.postMessage({ id: req.id, ok: true, stencils: names } satisfies OpfsWorkerResponse);
        break;
      }
      case 'saveVersion': {
        const byteLength = await withSyncHandle(
          'versions',
          `${req.docId}__${req.versionId}.vsdx`,
          (h) => writeSyncAtomic(h, req.vsdxBytes)
        );
        const indexName = `${req.docId}__index.json`;
        const existing = await readJsonIndex<OpfsVersionMeta>('versions', indexName);
        const meta: OpfsVersionMeta = {
          versionId: req.versionId,
          docId: req.docId,
          label: req.label,
          byteLength,
          createdAt: new Date().toISOString(),
        };
        const updated = [meta, ...existing.filter((v) => v.versionId !== req.versionId)].slice(0, 50);
        await writeJsonIndex('versions', indexName, updated);
        self.postMessage({ id: req.id, ok: true, byteLength, versions: updated } satisfies OpfsWorkerResponse);
        break;
      }
      case 'listVersions': {
        const versions = await readJsonIndex<OpfsVersionMeta>('versions', `${req.docId}__index.json`);
        self.postMessage({ id: req.id, ok: true, versions } satisfies OpfsWorkerResponse);
        break;
      }
      case 'loadVersion': {
        const bytes = await withSyncHandle(
          'versions',
          `${req.docId}__${req.versionId}.vsdx`,
          (h) => readSyncAll(h)
        );
        self.postMessage(
          { id: req.id, ok: true, bytes } satisfies OpfsWorkerResponse,
          { transfer: [bytes.buffer] }
        );
        break;
      }
      case 'saveWorkspace': {
        const byteLength = await withSyncHandle('workspaces', `${req.docId}.vsdx`, (h) =>
          writeSyncAtomic(h, req.vsdxBytes)
        );
        const existing = await readJsonIndex<OpfsWorkspaceMeta>('workspaces', '__index.json');
        const meta: OpfsWorkspaceMeta = {
          docId: req.docId,
          title: req.title,
          byteLength,
          updatedAt: new Date().toISOString(),
        };
        const updated = [meta, ...existing.filter((w) => w.docId !== req.docId)];
        await writeJsonIndex('workspaces', '__index.json', updated);
        self.postMessage({ id: req.id, ok: true, byteLength, workspaces: updated } satisfies OpfsWorkerResponse);
        break;
      }
      case 'listWorkspaces': {
        const workspaces = await readJsonIndex<OpfsWorkspaceMeta>('workspaces', '__index.json');
        self.postMessage({ id: req.id, ok: true, workspaces } satisfies OpfsWorkerResponse);
        break;
      }
      case 'loadWorkspace': {
        const bytes = await withSyncHandle('workspaces', `${req.docId}.vsdx`, (h) =>
          readSyncAll(h)
        );
        self.postMessage(
          { id: req.id, ok: true, bytes } satisfies OpfsWorkerResponse,
          { transfer: [bytes.buffer] }
        );
        break;
      }
    }
  } catch (err) {
    self.postMessage({
      id: req.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    } satisfies OpfsWorkerResponse);
  }
};

