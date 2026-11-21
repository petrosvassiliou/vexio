/**
 * Modular Firebase SDK v11+ Integration for ChromeOS PWA.
 *
 * Strictly architected for 100% unauthenticated offline-first execution:
 * - Never blocks app boot, file editing, validation, CAD import, or export when offline or signed out.
 * - Uses `persistentLocalCache({ tabManager: persistentMultipleTabManager() })` for IndexedDB
 *   Multi-Tab Offline Persistence of user settings, custom stencil metadata, version snapshots, and synced diagrams.
 * - Automatically connects to the local Firebase Emulator Suite when running on localhost / 127.0.0.1.
 */

import { initializeApp, type FirebaseApp } from 'firebase/app';
import {
  getAuth,
  connectAuthEmulator,
  GoogleAuthProvider,
  signInWithPopup,
  signInAnonymously,
  signOut,
  onAuthStateChanged,
  type Auth,
  type User,
} from 'firebase/auth';
import {
  initializeFirestore,
  persistentLocalCache,
  persistentMultipleTabManager,
  connectFirestoreEmulator,
  collection,
  doc,
  setDoc,
  getDoc,
  getDocs,
  type Firestore,
} from 'firebase/firestore';
import {
  getStorage,
  connectStorageEmulator,
  ref,
  uploadBytes,
  getBytes,
  type FirebaseStorage,
} from 'firebase/storage';

export interface CloudSyncState {
  initialized: boolean;
  online: boolean;
  user: User | null;
  lastSyncedIso: string | null;
}

export interface CloudDocMeta {
  docId: string;
  title: string;
  format: 'vsdx' | 'vsdm' | 'vssx' | 'vstx' | 'vsd';
  byteLength: number;
  updatedAt: string;
}

const FIREBASE_CONFIG = {
  apiKey: import.meta.env.VITE_FIREBASE_API_KEY ?? 'AIzaSyDANDxvTfJq2t_QZSK7ADO27z5_jO7Dzi0',
  authDomain: import.meta.env.VITE_FIREBASE_AUTH_DOMAIN ?? 'vexio-chromeos-suite.firebaseapp.com',
  projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID ?? 'vexio-chromeos-suite',
  storageBucket:
    import.meta.env.VITE_FIREBASE_STORAGE_BUCKET ?? 'vexio-chromeos-suite.firebasestorage.app',
  messagingSenderId: import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID ?? '186737172363',
  appId:
    import.meta.env.VITE_FIREBASE_APP_ID ?? '1:186737172363:web:576d2b268c59b3bf0b598e',
};

function toPlainBufferBytes(src: Uint8Array): Uint8Array<ArrayBuffer> {
  const copy = new Uint8Array(new ArrayBuffer(src.byteLength));
  copy.set(src);
  return copy;
}

function uint8ToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let i = 0; i < bytes.length; i += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunkSize));
  }
  return btoa(binary);
}

function base64ToUint8(b64: string): Uint8Array {
  const binary = atob(b64);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

export class VexioFirebaseSync {
  private app: FirebaseApp | null = null;
  private auth: Auth | null = null;
  private db: Firestore | null = null;
  private storage: FirebaseStorage | null = null;
  private currentUser: User | null = null;
  private listeners = new Set<(state: CloudSyncState) => void>();
  private lastSyncedIso: string | null = null;

  public initNonBlocking(): void {
    try {
      this.app = initializeApp(FIREBASE_CONFIG);
      this.db = initializeFirestore(this.app, {
        localCache: persistentLocalCache({
          tabManager: persistentMultipleTabManager(),
        }),
      });
      this.auth = getAuth(this.app);
      this.storage = getStorage(this.app);

      const isLocalEmulator =
        typeof window !== 'undefined' &&
        (window.location.hostname === '127.0.0.1' || window.location.hostname === 'localhost');

      if (isLocalEmulator) {
        connectAuthEmulator(this.auth, 'http://127.0.0.1:9099', { disableWarnings: true });
        connectFirestoreEmulator(this.db, '127.0.0.1', 8080);
        connectStorageEmulator(this.storage, '127.0.0.1', 9199);
      }

      onAuthStateChanged(this.auth, (user) => {
        this.currentUser = user;
        this.emit();
      });

      window.addEventListener('online', () => this.emit());
      window.addEventListener('offline', () => this.emit());
      this.emit();
    } catch {
      // Even if Firebase config or network is completely unreachable, Vexio runs 100% locally!
      this.emit();
    }
  }

  public getFirestoreInstance(): Firestore | null {
    return this.db;
  }

  public getCurrentUser(): User | null {
    return this.currentUser;
  }

  public subscribe(cb: (state: CloudSyncState) => void): () => void {
    this.listeners.add(cb);
    cb(this.snapshot());
    return () => this.listeners.delete(cb);
  }

  public snapshot(): CloudSyncState {
    return {
      initialized: this.db !== null,
      online: typeof navigator !== 'undefined' ? navigator.onLine : false,
      user: this.currentUser,
      lastSyncedIso: this.lastSyncedIso,
    };
  }

  private emit(): void {
    const snap = this.snapshot();
    for (const cb of this.listeners) cb(snap);
  }

  public async signInGoogleOrEmulator(): Promise<User | null> {
    if (!this.auth) return null;
    if (window.location.hostname === '127.0.0.1' || window.location.hostname === 'localhost') {
      const cred = await signInAnonymously(this.auth);
      return cred.user;
    }
    try {
      const provider = new GoogleAuthProvider();
      const cred = await signInWithPopup(this.auth, provider);
      return cred.user;
    } catch (err: unknown) {
      const code = (err as { code?: string })?.code ?? '';
      if (code === 'auth/operation-not-allowed') {
        const cred = await signInAnonymously(this.auth);
        return cred.user;
      }
      throw err;
    }
  }

  public async signOutUser(): Promise<void> {
    if (this.auth) {
      await signOut(this.auth);
    }
  }

  public async saveUserPreferences(prefs: Record<string, unknown>): Promise<void> {
    if (!this.db || !this.currentUser) return;
    const refDoc = doc(this.db, 'users', this.currentUser.uid, 'settings', 'workstation');
    const updatedAt = new Date().toISOString();
    await setDoc(
      refDoc,
      {
        schemaVersion: 1,
        updatedAt,
        ...prefs,
      },
      { merge: true }
    );
    this.lastSyncedIso = updatedAt;
    this.emit();
  }

  public async backupWorkspaceToCloud(
    docId: string,
    title: string,
    format: 'vsdx' | 'vsdm' | 'vssx' | 'vstx' | 'vsd',
    bytes: Uint8Array
  ): Promise<void> {
    if (!this.db || !this.currentUser) return;
    const uid = this.currentUser.uid;
    let storageBacked = false;
    if (this.storage) {
      try {
        const storageRef = ref(this.storage, `users/${uid}/workspaces/${docId}.${format}`);
        await uploadBytes(storageRef, toPlainBufferBytes(bytes));
        storageBacked = true;
      } catch {
        storageBacked = false;
      }
    }

    const updatedAt = new Date().toISOString();
    const payload: Record<string, unknown> = {
      title,
      format,
      byteLength: bytes.byteLength,
      storageBacked,
      updatedAt,
    };
    if (!storageBacked && bytes.byteLength <= 700_000) {
      payload.inlineBase64 = uint8ToBase64(bytes);
    }
    await setDoc(doc(this.db, 'users', uid, 'documents', docId), payload);
    this.lastSyncedIso = updatedAt;
    this.emit();
  }

  public async listCloudWorkspaces(): Promise<CloudDocMeta[]> {
    if (!this.db || !this.currentUser) return [];
    const uid = this.currentUser.uid;
    const qs = await getDocs(collection(this.db, 'users', uid, 'documents'));
    const out: CloudDocMeta[] = [];
    qs.forEach((snap) => {
      const d = snap.data();
      out.push({
        docId: snap.id,
        title: String(d.title ?? snap.id),
        format: (d.format as CloudDocMeta['format']) ?? 'vsdx',
        byteLength: Number(d.byteLength ?? 0),
        updatedAt: String(d.updatedAt ?? ''),
      });
    });
    return out.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  public async restoreWorkspaceFromCloud(
    docId: string,
    format: 'vsdx' | 'vsdm' | 'vssx' | 'vstx' | 'vsd'
  ): Promise<Uint8Array | null> {
    if (!this.db || !this.currentUser) return null;
    const uid = this.currentUser.uid;
    const metaSnap = await getDoc(doc(this.db, 'users', uid, 'documents', docId));
    if (!metaSnap.exists()) return null;
    const meta = metaSnap.data();
    if (meta.storageBacked !== false && this.storage) {
      try {
        const buf = await getBytes(ref(this.storage, `users/${uid}/workspaces/${docId}.${format}`));
        return new Uint8Array(buf);
      } catch {
        // Fall through to Firestore inlineBase64 snapshot if present
      }
    }
    if (typeof meta.inlineBase64 === 'string' && meta.inlineBase64.length > 0) {
      return base64ToUint8(meta.inlineBase64);
    }
    return null;
  }

  public async backupVersionToCloud(
    docId: string,
    versionId: string,
    label: string,
    vsdxBytes: Uint8Array
  ): Promise<void> {
    if (!this.db || !this.currentUser) return;
    const uid = this.currentUser.uid;
    let storageBacked = false;
    if (this.storage) {
      try {
        await uploadBytes(
          ref(this.storage, `users/${uid}/workspaces/${docId}__${versionId}.vsdx`),
          toPlainBufferBytes(vsdxBytes)
        );
        storageBacked = true;
      } catch {
        storageBacked = false;
      }
    }
    const createdAt = new Date().toISOString();
    const payload: Record<string, unknown> = {
      docId,
      label,
      byteLength: vsdxBytes.byteLength,
      storageBacked,
      createdAt,
    };
    if (!storageBacked && vsdxBytes.byteLength <= 700_000) {
      payload.inlineBase64 = uint8ToBase64(vsdxBytes);
    }
    await setDoc(doc(this.db, 'users', uid, 'versions', `${docId}__${versionId}`), payload);
    this.lastSyncedIso = createdAt;
    this.emit();
  }
}
