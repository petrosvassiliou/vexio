/**
 * Dual-Transport Real-Time Multi-User Co-Editing Coordinator (`collabAndVersions.ts`).
 *
 * Combines:
 * 1. `BroadcastChannel('vexio-collab-v1')` for zero-latency, 100% offline multi-window /
 *    multi-tab co-editing on ChromeOS (multiple Vexio windows side-by-side).
 * 2. Cloud Firestore `onSnapshot` (`/collabRooms/{roomId}/presence` & `/collabRooms/{roomId}/ops`)
 *    whenever signed in via Firebase Auth for real-time cross-device team presence & sync.
 * 3. Fine-grained Hybrid Logical Clock (HLC) Per-Cell CRDT delta packets (`crdtOps`) so
 *    simultaneous edits to distinct cells/text on the *same* shape merge with zero lost updates!
 */

import {
  collection,
  doc,
  onSnapshot,
  setDoc,
  type Unsubscribe,
} from 'firebase/firestore';
import type { VexioFirebaseSync } from '../firebase/firebaseClient';

export interface PeerPresence {
  peerId: string;
  peerName: string;
  color: string;
  cursorX: number;
  cursorY: number;
  pageId: number;
  selectedShapeIds: number[];
  updatedAtMs: number;
}

type BroadcastPacket =
  | {
      kind: 'presence';
      roomId: string;
      peer: PeerPresence;
    }
  | {
      kind: 'crdtOps';
      roomId: string;
      senderPeerId: string;
      opBatchId: string;
      crdtOpsJson: string;
    }
  | {
      kind: 'docSync';
      roomId: string;
      senderPeerId: string;
      vsdxBytes: Uint8Array;
      updatedAtIso: string;
    };

const PEER_COLORS = ['#ec4899', '#10b981', '#f59e0b', '#8b5cf6', '#06b6d4', '#f43f5e'];
const PEER_ROLES = ['Lead Architect', 'Systems Engineer', 'Process Analyst', 'CAD Specialist', 'Data Modeler'];

function uint8ToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

function base64ToUint8(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) {
    out[i] = bin.charCodeAt(i);
  }
  return out;
}

export class VexioCollabSession {
  public readonly peerId: string;
  public peerName: string;
  public readonly peerColor: string;

  private roomId: string;
  private firebaseSync: VexioFirebaseSync;
  private channel: BroadcastChannel | null = null;
  private peers = new Map<string, PeerPresence>();
  private firestoreUnsubs: Unsubscribe[] = [];
  private seenBatchIds = new Set<string>();
  private lastFirestoreCursorSentMs = 0;
  private lastAppliedRemoteOpIso = '';

  private onPeersChanged: (peers: PeerPresence[]) => void;
  private onRemoteCrdtOps: (crdtOpsJson: string, senderPeerId: string) => void;
  private onRemoteDocSync: (vsdxBytes: Uint8Array, senderPeerId: string) => void;

  constructor(opts: {
    roomId: string;
    firebaseSync: VexioFirebaseSync;
    onPeersChanged: (peers: PeerPresence[]) => void;
    onRemoteCrdtOps: (crdtOpsJson: string, senderPeerId: string) => void;
    onRemoteDocSync: (vsdxBytes: Uint8Array, senderPeerId: string) => void;
  }) {
    const rand = Math.floor(Math.random() * 900 + 100);
    this.peerId = `peer-${rand}-${Date.now().toString(36).slice(-4)}`;
    this.peerColor = PEER_COLORS[rand % PEER_COLORS.length];
    this.peerName = `${PEER_ROLES[rand % PEER_ROLES.length]} #${rand}`;
    this.roomId = opts.roomId;
    this.firebaseSync = opts.firebaseSync;
    this.onPeersChanged = opts.onPeersChanged;
    this.onRemoteCrdtOps = opts.onRemoteCrdtOps;
    this.onRemoteDocSync = opts.onRemoteDocSync;

    if (typeof BroadcastChannel !== 'undefined') {
      this.channel = new BroadcastChannel('vexio-collab-v1');
      this.channel.onmessage = (ev: MessageEvent<BroadcastPacket>) => {
        const pkt = ev.data;
        if (!pkt || pkt.roomId !== this.roomId) return;
        if (pkt.kind === 'presence' && pkt.peer.peerId !== this.peerId) {
          this.peers.set(pkt.peer.peerId, { ...pkt.peer, updatedAtMs: Date.now() });
          this.pruneAndNotify();
        } else if (pkt.kind === 'crdtOps' && pkt.senderPeerId !== this.peerId) {
          if (!this.seenBatchIds.has(pkt.opBatchId)) {
            this.seenBatchIds.add(pkt.opBatchId);
            this.onRemoteCrdtOps(pkt.crdtOpsJson, pkt.senderPeerId);
          }
        } else if (pkt.kind === 'docSync' && pkt.senderPeerId !== this.peerId) {
          if (pkt.updatedAtIso > this.lastAppliedRemoteOpIso) {
            this.lastAppliedRemoteOpIso = pkt.updatedAtIso;
            this.onRemoteDocSync(new Uint8Array(pkt.vsdxBytes), pkt.senderPeerId);
          }
        }
      };
    }

    setInterval(() => this.pruneAndNotify(), 6000);
    this.rebindFirestoreRoom();
  }

  public setRoom(newRoomId: string): void {
    if (this.roomId === newRoomId) return;
    this.roomId = newRoomId;
    this.peers.clear();
    this.onPeersChanged([]);
    this.rebindFirestoreRoom();
  }

  public rebindFirestoreRoom(): void {
    for (const u of this.firestoreUnsubs) u();
    this.firestoreUnsubs = [];

    const db = this.firebaseSync.getFirestoreInstance();
    const user = this.firebaseSync.getCurrentUser();
    if (!db || !user) return;

    if (user.email) {
      this.peerName = user.email.split('@')[0];
    }

    try {
      const presCol = collection(db, 'collabRooms', this.roomId, 'presence');
      const unsubPres = onSnapshot(
        presCol,
        (qs) => {
          qs.forEach((snap) => {
            if (snap.id === this.peerId) return;
            const d = snap.data();
            const updatedAtMs = Date.parse(String(d.updatedAt ?? '')) || Date.now();
            if (Date.now() - updatedAtMs <= 30000) {
              this.peers.set(snap.id, {
                peerId: snap.id,
                peerName: String(d.peerName ?? 'Collaborator'),
                color: String(d.color ?? '#ec4899'),
                cursorX: Number(d.cursorX ?? 0),
                cursorY: Number(d.cursorY ?? 0),
                pageId: Number(d.pageId ?? 1),
                selectedShapeIds: Array.isArray(d.selectedShapeIds) ? d.selectedShapeIds.map(Number) : [],
                updatedAtMs,
              });
            }
          });
          this.pruneAndNotify();
        },
        () => {}
      );

      const opsCol = collection(db, 'collabRooms', this.roomId, 'ops');
      const unsubOps = onSnapshot(
        opsCol,
        (qs) => {
          qs.docChanges().forEach((chg) => {
            if (chg.type === 'removed') return;
            const d = chg.doc.data();
            const senderPeerId = String(d.senderPeerId ?? '');
            if (!senderPeerId || senderPeerId === this.peerId) return;

            if (typeof d.crdtOpsJson === 'string' && !this.seenBatchIds.has(chg.doc.id)) {
              this.seenBatchIds.add(chg.doc.id);
              this.onRemoteCrdtOps(d.crdtOpsJson, senderPeerId);
            } else if (
              typeof d.vsdxBase64 === 'string' &&
              String(d.updatedAt ?? '') > this.lastAppliedRemoteOpIso
            ) {
              this.lastAppliedRemoteOpIso = String(d.updatedAt ?? '');
              this.onRemoteDocSync(base64ToUint8(d.vsdxBase64), senderPeerId);
            }
          });
        },
        () => {}
      );

      this.firestoreUnsubs.push(unsubPres, unsubOps);
    } catch {
      // Never block offline execution
    }
  }

  public broadcastCursor(cursorX: number, cursorY: number, pageId: number, selectedShapeIds: number[]): void {
    const peer: PeerPresence = {
      peerId: this.peerId,
      peerName: this.peerName,
      color: this.peerColor,
      cursorX,
      cursorY,
      pageId,
      selectedShapeIds,
      updatedAtMs: Date.now(),
    };

    this.channel?.postMessage({
      kind: 'presence',
      roomId: this.roomId,
      peer,
    } satisfies BroadcastPacket);

    const db = this.firebaseSync.getFirestoreInstance();
    const user = this.firebaseSync.getCurrentUser();
    const now = Date.now();
    if (db && user && now - this.lastFirestoreCursorSentMs > 220) {
      this.lastFirestoreCursorSentMs = now;
      setDoc(doc(db, 'collabRooms', this.roomId, 'presence', this.peerId), {
        uid: user.uid,
        peerName: this.peerName,
        color: this.peerColor,
        cursorX,
        cursorY,
        pageId,
        selectedShapeIds,
        updatedAt: new Date(now).toISOString(),
      }).catch(() => {});
    }
  }

  public broadcastCrdtOps(crdtOpsJson: string): void {
    if (!crdtOpsJson || crdtOpsJson === '[]') return;
    const opBatchId = `${this.peerId}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
    this.seenBatchIds.add(opBatchId);

    this.channel?.postMessage({
      kind: 'crdtOps',
      roomId: this.roomId,
      senderPeerId: this.peerId,
      opBatchId,
      crdtOpsJson,
    } satisfies BroadcastPacket);

    const db = this.firebaseSync.getFirestoreInstance();
    const user = this.firebaseSync.getCurrentUser();
    if (db && user && crdtOpsJson.length < 600_000) {
      setDoc(doc(db, 'collabRooms', this.roomId, 'ops', opBatchId), {
        uid: user.uid,
        senderPeerId: this.peerId,
        crdtOpsJson,
        updatedAt: new Date().toISOString(),
      }).catch(() => {});
    }
  }

  public broadcastDocumentMutation(vsdxBytes: Uint8Array): void {
    const updatedAtIso = new Date().toISOString();
    this.lastAppliedRemoteOpIso = updatedAtIso;

    this.channel?.postMessage({
      kind: 'docSync',
      roomId: this.roomId,
      senderPeerId: this.peerId,
      vsdxBytes,
      updatedAtIso,
    } satisfies BroadcastPacket);

    const db = this.firebaseSync.getFirestoreInstance();
    const user = this.firebaseSync.getCurrentUser();
    if (db && user && vsdxBytes.byteLength < 500_000) {
      setDoc(doc(db, 'collabRooms', this.roomId, 'ops', 'latest'), {
        uid: user.uid,
        senderPeerId: this.peerId,
        vsdxBase64: uint8ToBase64(vsdxBytes),
        updatedAt: updatedAtIso,
      }).catch(() => {});
    }
  }

  private pruneAndNotify(): void {
    const cutoff = Date.now() - 25000;
    for (const [id, p] of this.peers.entries()) {
      if (p.updatedAtMs < cutoff) {
        this.peers.delete(id);
      }
    }
    this.onPeersChanged(Array.from(this.peers.values()));
  }
}
