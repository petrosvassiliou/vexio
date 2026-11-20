import './styles/m3-expressive.css';
import { VexioFirebaseSync, type CloudDocMeta, type CloudSyncState } from './firebase/firebaseClient';
import {
  bindWindowControlsOverlay,
  copyDiagramVectorToClipboard,
  enumerateChromeOsSystemFonts,
  openLocalDiagramFile,
  pickLocalBinaryFile,
  registerChromeOsFileLaunchHandler,
  saveLocalDiagramInPlace,
} from './platform/chromeosIntegration';
import { VexioCollabSession, type PeerPresence } from './platform/collabAndVersions';
import type { EditableVertexDto, EngineSnapshot, SmartGuideLine } from './workers/engine.worker';
import type { OpfsVersionMeta, OpfsWorkspaceMeta } from './workers/opfs-journal.worker';

type ActiveTool = 'pointer' | 'connector' | 'rect' | 'freeform' | 'pen' | 'highlighter';
type InspectorTab = 'props' | 'layers' | 'comments' | 'validate';
type ActiveModal = null | 'backstage' | 'vba' | 'sql' | 'data' | 'org' | 'dataviz' | 'cloud' | 'compare' | 'link';
type ResizeHandleDir = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

interface StarterTemplateCard {
  id: string;
  title: string;
  category: string;
  desc: string;
}

const STARTER_TEMPLATES: StarterTemplateCard[] = [
  { id: 'bpmn_fulfillment', title: 'BPMN 2.0 Order Fulfillment', category: 'Business Process', desc: 'Gateways, tasks & sequence flows ready for BPMN validation' },
  { id: 'cloud_microservices', title: 'Cloud Kubernetes Architecture', category: 'Cloud (AWS/GCP/Azure)', desc: 'Edge CDN, K8s cluster, serverless & multi-zone VPC container' },
  { id: 'sql_ecommerce_erd', title: "E-Commerce Crow's Foot ERD", category: 'Database Schema', desc: '4 relational tables with PK/FK Crow’s Foot connectors' },
  { id: 'executive_org_chart', title: 'Executive Organization Chart', category: 'Org Chart', desc: 'Role-styled reporting hierarchy built from roster CSV' },
  { id: 'release_gantt', title: 'Product Launch Gantt Schedule', category: 'Project Schedule', desc: 'Proportional task bars, completion % fills & FS dependencies' },
  { id: 'project_timeline', title: 'Annual Architecture Roadmap', category: 'Timeline', desc: 'Executive spine with major/minor diamond milestones' },
  { id: 'datacenter_floorplan', title: 'Data Center Floor Plan & Rack', category: 'Facilities & Network', desc: '1/4"=1\'0" scaled rooms with live SqFt + 42U spine rack' },
  { id: 'blank', title: 'Blank Visio Drawing', category: 'General', desc: 'Clean 11 × 8.5 in landscape sheet with default layers' },
];

const FALLBACK_STENCILS: Record<string, string[]> = {
  'BPMN 2.0': ['Start Event', 'Task', 'Exclusive Gateway', 'Parallel Gateway', 'Intermediate Event', 'End Event'],
  'Basic Flowchart': ['Process', 'Decision', 'Start/End', 'Data (I/O)', 'Subprocess'],
};

const DEFAULT_VBA_SCRIPT = `Sub BuildCluster()
  Begin UserForm DeployForm
    Caption = "Cluster Provisioning"
    Control TextBox txtEnv = "Prod"
  End
  For i = 1 To 3
    Set node = ActivePage.DrawRectangle(1.2 + i * 2.2, 2.2, 2.7 + i * 2.2, 3.1)
    If i = 2 Then
      node.Text = "Gateway (" & DeployForm.txtEnv & ")"
    Else
      node.Text = "Microservice " & i
    End If
    node.Cells("FillForegnd") = "RGB(219,234,254)"
  Next i
End Sub`;

const DEFAULT_SQL_DDL = `CREATE TABLE organizations (
  id INT PRIMARY KEY,
  name VARCHAR NOT NULL,
  plan_tier VARCHAR NOT NULL
);

CREATE TABLE workspaces (
  id INT PRIMARY KEY,
  org_id INT NOT NULL REFERENCES organizations(id),
  title VARCHAR NOT NULL,
  region VARCHAR
);

CREATE TABLE diagrams (
  id INT PRIMARY KEY,
  workspace_id INT NOT NULL REFERENCES workspaces(id),
  format VARCHAR NOT NULL
);`;

const DEFAULT_TELEMETRY_CSV = `Label,Status,Load
Validate Order & Inventory,Healthy,88
In Stock?,Warning,52`;

const DEFAULT_ORG_CSV = `Name,Title,Manager,Department
Elena Rostova,Chief Executive Officer,,Executive
Marcus Vance,VP Systems Engineering,Elena Rostova,Engineering
Priya Nair,VP Product & Design,Elena Rostova,Product
Liam O'Connor,Rust Graphics Lead,Marcus Vance,Core Engine
Sofia Lindqvist,Platform UI Lead,Priya Nair,Design Systems`;

const DEFAULT_DATAVIZ_CSV = `Process Step ID,Process Step Description,Next Step ID,Connector Label,Shape Type,Function,Owner
S1,Receive Customer Request,S2,,Start,Customer Portal,Support
S2,Automated Fraud Check,S3;S4,Pass;Review,Decision,Risk Engine,SecOps
S3,Provision Subscription,S5,,Process,Billing Core,Finance
S4,Manual Compliance Audit,S5,Approved,Subprocess,Risk Engine,Compliance
S5,Order Completed,,,End,Customer Portal,Support`;

const DEFAULT_CLOUD_JSON = JSON.stringify(
  {
    provider: 'aws',
    vpcs: [{ id: 'vpc-prod', name: 'Production Multi-AZ VPC', cidr: '10.20.0.0/16' }],
    resources: [
      { id: 'cf-edge', name: 'CloudFront Edge', kind: 'cdn', vpc: 'vpc-prod' },
      { id: 'eks-api', name: 'EKS Rust Core', kind: 'compute', vpc: 'vpc-prod' },
      { id: 'aurora-pg', name: 'Aurora PostgreSQL', kind: 'database', vpc: 'vpc-prod' },
    ],
    edges: [
      { from: 'cf-edge', to: 'eks-api', label: 'HTTPS/3' },
      { from: 'eks-api', to: 'aurora-pg', label: 'TLS 1.3' },
    ],
  },
  null,
  2
);

interface DiagramDiffReportDto {
  added_count: number;
  removed_count: number;
  modified_count: number;
  entries: Array<{ page_name: string; shape_id: number; shape_name: string; kind: unknown; summary: string }>;
  diff_overlay_svg: string;
}

class VexioChromeOsApp {
  private engineWorker: Worker;
  private opfsWorker: Worker;
  private firebaseSync = new VexioFirebaseSync();
  private collabSession!: VexioCollabSession;
  private rpcSeq = 1;
  private pendingRpc = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();
  private pendingOpfs = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();

  // Persistent WebGPU OffscreenCanvas element preserved across DOM re-renders
  private wgpuCanvasEl: HTMLCanvasElement;
  private offscreenTransferred = false;
  private wgpuAttached = false;

  private docId = 'workspace-default';
  private activeFileHandle: FileSystemFileHandle | null = null;
  private currentFileName = 'Architecture-Diagram.vsdx';
  private snapshot: EngineSnapshot | null = null;
  private stencilCatalog: Record<string, string[]> = FALLBACK_STENCILS;
  private cloudState: CloudSyncState = {
    initialized: false,
    online: navigator.onLine,
    user: null,
    lastSyncedIso: null,
  };

  private peers: PeerPresence[] = [];
  private opfsVersions: OpfsVersionMeta[] = [];
  private opfsWorkspaces: OpfsWorkspaceMeta[] = [];
  private cloudWorkspaces: CloudDocMeta[] = [];
  private syncDebounceTimer: number | null = null;

  private activeRibbonTab = 'Home';
  private activeTool: ActiveTool = 'pointer';
  private activeInspectorTab: InspectorTab = 'props';
  private activeModal: ActiveModal = null;
  private modalOutputMessage = '';
  private vbaDraft = DEFAULT_VBA_SCRIPT;
  private sqlDraft = DEFAULT_SQL_DDL;
  private csvDraft = DEFAULT_TELEMETRY_CSV;
  private orgCsvDraft = DEFAULT_ORG_CSV;
  private dataVizDraft = DEFAULT_DATAVIZ_CSV;
  private cloudJsonDraft = DEFAULT_CLOUD_JSON;
  private versionLabelDraft = 'Milestone Checkpoint';
  private selectedThemeSlug = 'modern';
  private selectedVariantIx = 0;
  private linkAddressDraft = 'https://docs.vexio.dev/runbook';
  private linkSubAddressDraft = '';
  private linkScreentipDraft = 'Click or hover to inspect architecture SLA & owner';
  private latestDiffReport: DiagramDiffReportDto | null = null;

  private activeStencilPack = 'BPMN 2.0';
  private stencilFilter = '';
  private selectedShapeIds: number[] = [];
  private showShapeSheet = true;
  private showRulers = true;
  private showMinimap = true;
  private showVertexGrips = false;
  private activeShapeVertices: EditableVertexDto[] = [];
  private zoomPct = 100;
  private cursorCoordsIn: [number, number] = [0, 0];
  private systemFonts: string[] = [];
  private smartGuideReqInFlight = false;
  private latestSmartGuides: SmartGuideLine[] = [];
  private latestSnappedPin: [number, number] | null = null;

  private dragState:
    | null
    | {
        mode: 'move' | 'lasso' | 'resize' | 'rotate' | 'connect' | 'rect' | 'freeform' | 'ink' | 'control' | 'vertex';
        startPageIn: [number, number];
        curPageIn: [number, number];
        shapeId?: number;
        origPin?: [number, number];
        origSize?: [number, number];
        origAngleDeg?: number;
        resizeHandle?: ResizeHandleDir;
        controlRowName?: string;
        geomIndex?: number;
        vertexRowIx?: number;
        freeformPoints?: Array<[number, number]>;
        inkPoints?: Array<{ x: number; y: number; pressure: number; tilt_x: number; tilt_y: number }>;
      } = null;

  constructor() {
    this.wgpuCanvasEl = document.createElement('canvas');
    this.wgpuCanvasEl.className = 'wgpu-viewport-canvas';
    this.wgpuCanvasEl.width = 1056;
    this.wgpuCanvasEl.height = 816;

    this.engineWorker = new Worker(new URL('./workers/engine.worker.ts', import.meta.url), {
      type: 'module',
    });
    this.opfsWorker = new Worker(new URL('./workers/opfs-journal.worker.ts', import.meta.url), {
      type: 'module',
    });

    this.engineWorker.onmessage = (ev) => {
      const { id, ok, error, ...rest } = ev.data;
      const waiter = this.pendingRpc.get(id);
      if (waiter) {
        this.pendingRpc.delete(id);
        ok ? waiter.resolve(rest) : waiter.reject(new Error(error));
      }
    };

    this.opfsWorker.onmessage = (ev) => {
      const { id, ok, error, ...rest } = ev.data;
      const waiter = this.pendingOpfs.get(id);
      if (waiter) {
        this.pendingOpfs.delete(id);
        ok ? waiter.resolve(rest) : waiter.reject(new Error(error));
      }
    };
  }

  private get primarySelectedId(): number | null {
    return this.selectedShapeIds[0] ?? null;
  }

  private setSingleSelection(id: number | null): void {
    this.selectedShapeIds = id ? [id] : [];
    if (this.showVertexGrips && id) {
      this.refreshActiveVertices(id);
    } else {
      this.activeShapeVertices = [];
    }
  }

  private async refreshActiveVertices(shapeId: number): Promise<void> {
    try {
      const res = await this.callEngine('listShapeVertices', { shapeId });
      this.activeShapeVertices = res.vertices ?? [];
      this.render();
    } catch {
      this.activeShapeVertices = [];
    }
  }

  private callEngine(cmd: string, payload?: Record<string, unknown>, transfer?: Transferable[]): Promise<any> {
    const id = this.rpcSeq++;
    return new Promise((resolve, reject) => {
      this.pendingRpc.set(id, { resolve, reject });
      this.engineWorker.postMessage({ id, cmd, payload }, transfer ?? []);
    });
  }

  private callOpfs(op: string, extra?: Record<string, unknown>, transfer?: Transferable[]): Promise<any> {
    const id = this.rpcSeq++;
    return new Promise((resolve, reject) => {
      this.pendingOpfs.set(id, { resolve, reject });
      this.opfsWorker.postMessage({ id, op, ...extra }, transfer ?? []);
    });
  }

  private persistMutationSideEffects(wal?: Uint8Array): void {
    if (wal) {
      this.callOpfs('writeWal', { docId: this.docId, bytes: wal }, [wal.buffer]).catch(() => {});
    }
    if (this.syncDebounceTimer !== null) {
      window.clearTimeout(this.syncDebounceTimer);
    }
    this.syncDebounceTimer = window.setTimeout(async () => {
      try {
        const { crdtOpsJson } = await this.callEngine('flushCrdtOps');
        if (crdtOpsJson && crdtOpsJson !== '[]') {
          await this.collabSession?.broadcastCrdtOps(crdtOpsJson);
        }
        const { bytes } = await this.callEngine('saveFormat', { ext: 'vsdx' });
        const copyForWorkspace = new Uint8Array(bytes);
        await this.callOpfs(
          'saveWorkspace',
          { docId: this.docId, title: this.currentFileName, vsdxBytes: copyForWorkspace },
          [copyForWorkspace.buffer]
        );
      } catch {
        // Ignore transient background persistence errors
      }
    }, 110);
  }

  public async boot(): Promise<void> {
    if ('serviceWorker' in navigator) {
      navigator.serviceWorker.register('/sw.js').catch(() => {});
    }

    bindWindowControlsOverlay();
    registerChromeOsFileLaunchHandler(async (opened) => {
      this.activeFileHandle = opened.handle;
      this.currentFileName = opened.name;
      const res = await this.callEngine('openBytes', { bytes: opened.bytes, ext: opened.ext });
      this.snapshot = res.snapshot;
      this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
      this.render();
    });

    this.collabSession = new VexioCollabSession({
      roomId: this.docId,
      firebaseSync: this.firebaseSync,
      onPeersChanged: (peers) => {
        this.peers = peers;
        this.renderPeersOverlayOnly();
      },
      onRemoteDocSync: async (vsdxBytes) => {
        const res = await this.callEngine('openBytes', { bytes: vsdxBytes, ext: 'vsdx' });
        this.snapshot = res.snapshot;
        this.render();
      },
      onRemoteCrdtOps: async (crdtOpsJson) => {
        const res = await this.callEngine('applyRemoteCrdtOps', { crdtOpsJson });
        if (res.applied > 0) {
          this.snapshot = res.snapshot;
          this.render();
        }
      },
    });

    this.firebaseSync.subscribe((st) => {
      this.cloudState = st;
      this.collabSession?.rebindFirestoreRoom();
      this.renderStatusChipsOnly();
    });
    this.firebaseSync.initNonBlocking();

    const sharedBuffer =
      typeof SharedArrayBuffer !== 'undefined' && window.crossOriginIsolated
        ? new SharedArrayBuffer(4096)
        : undefined;

    const initRes = await this.callEngine('init', { sharedBuffer, seedDemo: true });
    this.snapshot = initRes.snapshot;
    if (initRes.stencils && Object.keys(initRes.stencils).length > 0) {
      this.stencilCatalog = initRes.stencils;
    }
    const initialShapeId = this.snapshot?.shapes[1]?.id ?? this.snapshot?.shapes[0]?.id ?? null;
    this.setSingleSelection(initialShapeId);

    // Transfer OffscreenCanvas to Rust wgpu WebGPU/WebGL2 worker pipeline once!
    if (!this.offscreenTransferred && 'transferControlToOffscreen' in this.wgpuCanvasEl) {
      try {
        this.offscreenTransferred = true;
        const offscreen = this.wgpuCanvasEl.transferControlToOffscreen();
        const dpi = window.devicePixelRatio || 1;
        const attachRes = await this.callEngine(
          'attachOffscreenCanvas',
          { canvas: offscreen, width: Math.round(1056 * dpi), height: Math.round(816 * dpi), dpi },
          [offscreen]
        );
        this.wgpuAttached = Boolean(attachRes.attached);
        if (attachRes.snapshot) {
          this.snapshot = attachRes.snapshot;
        }
      } catch {
        this.wgpuAttached = false;
      }
    }

    enumerateChromeOsSystemFonts().then((fonts) => {
      this.systemFonts = fonts;
    });

    this.bindKeyboardShortcuts();
    this.render();
  }

  private bindKeyboardShortcuts(): void {
    window.addEventListener('keydown', async (e) => {
      const target = e.target as HTMLElement | null;
      const inInput = target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA');

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        this.snapshot = (await this.callEngine(e.shiftKey ? 'redo' : 'undo')).snapshot;
        this.persistMutationSideEffects();
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') {
        e.preventDefault();
        this.snapshot = (await this.callEngine('redo')).snapshot;
        this.persistMutationSideEffects();
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        await this.handleSaveInPlace('vsdx');
      } else if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        this.selectedShapeIds = (this.snapshot?.shapes ?? [])
          .filter((s) => s.kind !== 'Connector1D')
          .map((s) => s.id);
        await this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds });
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.key === '1') {
        e.preventDefault();
        this.activeTool = 'pointer';
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.key === '3') {
        e.preventDefault();
        this.activeTool = 'connector';
        this.render();
      } else if (!inInput && (e.key === 'Delete' || e.key === 'Backspace') && this.selectedShapeIds.length > 0) {
        e.preventDefault();
        const res = await this.callEngine('deleteShape', { shapeIds: this.selectedShapeIds });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
        this.render();
      } else if (e.shiftKey && e.key === 'F5') {
        e.preventDefault();
        this.showShapeSheet = !this.showShapeSheet;
        this.render();
      }
    });
  }

  private async openBackstageModal(): Promise<void> {
    this.activeModal = 'backstage';
    this.modalOutputMessage = '';
    try {
      const [vRes, wRes, cwRes] = await Promise.all([
        this.callOpfs('listVersions', { docId: this.docId }),
        this.callOpfs('listWorkspaces'),
        this.firebaseSync.listCloudWorkspaces(),
      ]);
      this.opfsVersions = vRes.versions ?? [];
      this.opfsWorkspaces = wRes.workspaces ?? [];
      this.cloudWorkspaces = cwRes;
    } catch {
      // Offline OPFS fallback
    }
    this.render();
  }

  private async handleOpenDisk(): Promise<void> {
    const opened = await openLocalDiagramFile();
    if (!opened) return;
    this.activeFileHandle = opened.handle;
    this.currentFileName = opened.name;
    const res = await this.callEngine('openBytes', { bytes: opened.bytes, ext: opened.ext });
    this.snapshot = res.snapshot;
    this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
    this.persistMutationSideEffects();
    this.render();
  }

  private async handleSaveInPlace(ext: string): Promise<void> {
    const { bytes, wal } = await this.callEngine('saveFormat', { ext });
    this.persistMutationSideEffects(wal);
    const targetName =
      ext === 'vsdx'
        ? this.currentFileName
        : `${this.currentFileName.replace(/\.[^.]+$/, '')}.${ext}`;
    const handle = await saveLocalDiagramInPlace(
      ext === 'vsdx' ? this.activeFileHandle : null,
      targetName,
      bytes
    );
    if (ext === 'vsdx' && handle) {
      this.activeFileHandle = handle;
    }
    if (ext === 'vsdx' && this.cloudState.user) {
      await this.firebaseSync.backupWorkspaceToCloud(this.docId, this.currentFileName, 'vsdx', bytes);
    }
  }

  private clientToPageInches(hostEl: HTMLElement, clientX: number, clientY: number): [number, number] {
    const rect = hostEl.getBoundingClientRect();
    const pageW = this.snapshot?.pageWidthIn ?? 11.0;
    const pageH = this.snapshot?.pageHeightIn ?? 8.5;
    const u = Math.max(0, Math.min(1, (clientX - rect.left) / Math.max(1, rect.width)));
    const v = Math.max(0, Math.min(1, (clientY - rect.top) / Math.max(1, rect.height)));
    return [u * pageW, (1 - v) * pageH];
  }

  private renderStatusChipsOnly(): void {
    const el = document.getElementById('cloud-sync-chip');
    if (!el) return;
    el.className = `status-chip ${this.cloudState.online ? '' : 'offline'} no-drag`;
    el.textContent = this.cloudState.online
      ? this.cloudState.user
        ? `Firebase Synced (${this.cloudState.user.uid.slice(0, 6)})`
        : 'Offline-Ready • OPFS Active'
      : '100% Offline Mode • OPFS WAL Active';
  }

  private renderPeersOverlayOnly(): void {
    const stackEl = document.getElementById('peer-avatar-stack');
    if (stackEl) {
      stackEl.innerHTML = this.peers
        .map(
          (p) =>
            `<span class="peer-avatar-pill" style="background:${p.color}" title="${p.peerName} (Live Co-Editing)">● ${p.peerName}</span>`
        )
        .join('');
    }
    const layerEl = document.getElementById('peer-cursor-layer');
    if (layerEl && this.snapshot) {
      const ppi = 96;
      const pageH = this.snapshot.pageHeightIn;
      const activePid = this.snapshot.activePageId;
      layerEl.innerHTML = this.peers
        .filter((p) => p.pageId === activePid)
        .map((p) => {
          const lx = p.cursorX * ppi;
          const ty = (pageH - p.cursorY) * ppi;
          return `
            <div class="peer-cursor-pin" style="left:${lx.toFixed(1)}px;top:${ty.toFixed(1)}px">
              <svg width="14" height="18" viewBox="0 0 14 18" style="display:block">
                <path d="M1 1 L13 10 L7 11 L4 17 Z" fill="${p.color}" stroke="#ffffff" stroke-width="1.4"/>
              </svg>
              <span class="peer-cursor-badge" style="background:${p.color}">${p.peerName}</span>
            </div>
          `;
        })
        .join('');
    }
  }

  private renderContextualRibbon(snap: EngineSnapshot): string {
    const selDisabled = this.selectedShapeIds.length === 0 ? 'disabled' : '';
    const multiDisabled = this.selectedShapeIds.length < 2 ? 'disabled' : '';
    const distDisabled = this.selectedShapeIds.length < 3 ? 'disabled' : '';

    switch (this.activeRibbonTab) {
      case 'Home':
        return `
          <span class="ribbon-group-label">Tools</span>
          <button class="m3-btn ${this.activeTool === 'pointer' ? 'active' : ''}" data-tool="pointer">Pointer / Lasso</button>
          <button class="m3-btn ${this.activeTool === 'connector' ? 'active' : ''}" data-tool="connector">Connector</button>
          <button class="m3-btn ${this.activeTool === 'rect' ? 'active' : ''}" data-tool="rect">Rectangle</button>
          <button class="m3-btn ${this.activeTool === 'freeform' ? 'active' : ''}" data-tool="freeform" title="Draw Smooth NURBS / Cubic Bezier Freeform Curve">Freeform Curve</button>
          <button class="m3-btn" id="btn-delete-sel" ${selDisabled}>Delete (${this.selectedShapeIds.length || 0})</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Align & Distribute</span>
          <button class="m3-btn" data-align="left" ${multiDisabled} title="Align Left">⫯ Left</button>
          <button class="m3-btn" data-align="center" ${multiDisabled} title="Align Center">⫰ Center</button>
          <button class="m3-btn" data-align="right" ${multiDisabled} title="Align Right">⫱ Right</button>
          <button class="m3-btn" data-align="middle" ${multiDisabled} title="Align Middle">⫲ Middle</button>
          <button class="m3-btn" data-align="distribute_h" ${distDisabled} title="Distribute Horizontally">⇔ Dist H</button>
          <button class="m3-btn" data-align="distribute_v" ${distDisabled} title="Distribute Vertically">⇕ Dist V</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Fill</span>
          ${['219,234,254', '220,252,231', '254,243,199', '243,232,255', '254,226,226']
            .map(
              (rgb) =>
                `<button class="swatch-btn" data-fill-rgb="${rgb}" style="background:rgb(${rgb})" title="Fill RGB(${rgb})"></button>`
            )
            .join('')}
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Export</span>
          <button class="m3-btn" id="btn-copy-svg">Copy SVG</button>
          <button class="m3-btn" id="btn-export-pdf">PDF/A</button>
          <button class="m3-btn" id="btn-export-pptx">Slides (.pptx)</button>
          <button class="m3-btn" id="btn-export-docx">Word Spec (.docx)</button>
          <button class="m3-btn" id="btn-export-dxf">DXF</button>
          <button class="m3-btn" id="btn-export-vsd">Legacy .vsd</button>
        `;
      case 'Insert':
        return `
          <span class="ribbon-group-label">Diagram Parts</span>
          <button class="m3-btn" id="btn-ins-container">Insert Container</button>
          <button class="m3-btn" id="btn-ins-callout" ${this.primarySelectedId ? '' : 'disabled'}>Attach Callout</button>
          <button class="m3-btn" id="btn-ins-swimlanes">Cross-Functional Swimlanes</button>
          <button class="m3-btn" id="btn-ins-image">+ Cropped Image</button>
          <button class="m3-btn" id="btn-ins-ctrl-handle" ${this.primarySelectedId ? '' : 'disabled'}>+ Yellow Control Handle</button>
          <button class="m3-btn" id="btn-open-link-modal" ${this.primarySelectedId ? '' : 'disabled'}>Hyperlink & ScreenTip...</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Architecture, Rack & CAD</span>
          <button class="m3-btn" id="btn-ins-room">Room Space (Live SqFt)</button>
          <button class="m3-btn" id="btn-ins-rack">42U Rack + Equipment</button>
          <button class="m3-btn" id="btn-ins-cad">Import CAD (.dwg / .dxf)</button>
        `;
      case 'Design':
        return `
          <span class="ribbon-group-label">6 Themes × 4 Variants</span>
          <select class="cell-input" id="theme-slug-select" style="width:128px">
            ${['modern', 'professional', 'vibrant', 'blueprint', 'monochrome', 'hand_drawn']
              .map(
                (t) =>
                  `<option value="${t}" ${this.selectedThemeSlug === t ? 'selected' : ''}>Theme: ${t}</option>`
              )
              .join('')}
          </select>
          <select class="cell-input" id="theme-variant-select" style="width:92px">
            ${[0, 1, 2, 3]
              .map(
                (v) =>
                  `<option value="${v}" ${this.selectedVariantIx === v ? 'selected' : ''}>Variant ${v + 1}</option>`
              )
              .join('')}
          </select>
          <button class="m3-btn active" id="btn-apply-theme">Apply Theme</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Visual Effects</span>
          <button class="m3-btn" data-vis-preset="grad_linear" ${selDisabled}>Linear Grad</button>
          <button class="m3-btn" data-vis-preset="grad_radial" ${selDisabled}>Radial Grad</button>
          <button class="m3-btn" data-vis-preset="hatch_diag" ${selDisabled}>Hatch</button>
          <button class="m3-btn" data-vis-preset="shadow_soft" ${selDisabled}>Drop Shadow</button>
          <button class="m3-btn" data-vis-preset="bevel_3d" ${selDisabled}>3D Bevel</button>
          <button class="m3-btn" data-vis-preset="sketchy" ${selDisabled}>Sketchy</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Auto-Layout & Title Block</span>
          <button class="m3-btn" data-layout="tb">Top-Down</button>
          <button class="m3-btn" data-layout="lr">Left-Right</button>
          <button class="m3-btn" data-layout="radial">Radial</button>
          <button class="m3-btn" id="btn-iso-title-block">ISO Title Block Border</button>
          <button class="m3-btn" data-pagesize="11,8.5,0">11×8.5 Arc</button>
          <button class="m3-btn" data-pagesize="17,11,2">17×11 Tabloid</button>
        `;
      case 'Data':
        return `
          <span class="ribbon-group-label">Plan 2 Data Ecosystem</span>
          <button class="m3-btn active" id="btn-open-data-modal">CSV / SQLite & Data Graphics...</button>
          <button class="m3-btn active" id="btn-open-dataviz-modal">Data Visualizer (Excel ⇄ Swimlanes)...</button>
          <button class="m3-btn active" id="btn-open-cloud-modal">Cloud Discovery (AWS/Azure/GCP)...</button>
          <button class="m3-btn active" id="btn-open-org-modal">Org Chart (CSV / Entra / LDIF)...</button>
          <button class="m3-btn" id="btn-export-pbi">Power BI (.pbi.json)</button>
          <button class="m3-btn" id="btn-export-bom">BOM Inventory (.xlsx)</button>
        `;
      case 'Process':
        return `
          <span class="ribbon-group-label">Subprocess & Workflows</span>
          <button class="m3-btn active" id="btn-create-subprocess" ${this.primarySelectedId ? '' : 'disabled'}>Create / Drill-Down Subprocess</button>
          <button class="m3-btn" id="btn-export-flow">Power Automate (.flow.json)</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Rule Validation</span>
          <button class="m3-btn" data-validate="bpmn">Check BPMN 2.0</button>
          <button class="m3-btn" data-validate="uml">Check UML 2.5</button>
          <button class="m3-btn" data-validate="workflow">Check SharePoint</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Schedules</span>
          <button class="m3-btn" id="btn-gen-pert">PERT Critical Path</button>
          <button class="m3-btn" id="btn-gen-gantt">Gantt Chart</button>
          <button class="m3-btn" id="btn-gen-timeline">Timeline</button>
        `;
      case 'Review':
        return `
          <span class="ribbon-group-label">USI Active Stylus Ink</span>
          <button class="m3-btn ${this.activeTool === 'pen' ? 'active' : ''}" data-tool="pen">Stylus Pen Ink</button>
          <button class="m3-btn ${this.activeTool === 'highlighter' ? 'active' : ''}" data-tool="highlighter">Yellow Highlighter</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Compare, Comments & CRDT Co-Editing (${snap.comments.length} notes • ${this.peers.length + 1} live)</span>
          <button class="m3-btn active" id="btn-open-compare-modal">Diagram Compare (Visual Diff)...</button>
          <button class="m3-btn" id="btn-add-quick-comment">Add Review Comment</button>
          <button class="m3-btn" id="btn-open-versions">Version History Snapshots...</button>
        `;
      case 'View':
        return `
          <span class="ribbon-group-label">Show / Panes</span>
          <button class="m3-btn ${this.showShapeSheet ? 'active' : ''}" id="btn-toggle-ss">ShapeSheet Split-View (Shift+F5)</button>
          <button class="m3-btn ${this.showMinimap ? 'active' : ''}" id="btn-toggle-minimap">Pan & Zoom Minimap</button>
          <button class="m3-btn ${this.showRulers ? 'active' : ''}" id="btn-toggle-rulers">Inch Rulers</button>
          <button class="m3-btn" id="btn-open-layers">Layer Manager (${snap.layers.length})</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Zoom & Crash Recovery</span>
          <button class="m3-btn" id="btn-fit-window">Fit 100%</button>
          <button class="m3-btn" id="btn-recover-opfs">Replay OPFS WAL Journal</button>
        `;
      case 'Developer':
      default:
        return `
          <span class="ribbon-group-label">Shape Design & Boolean Ops</span>
          <button class="m3-btn" data-bool-op="union" ${multiDisabled}>Union</button>
          <button class="m3-btn" data-bool-op="combine" ${multiDisabled}>Combine</button>
          <button class="m3-btn" data-bool-op="fragment" ${multiDisabled}>Fragment</button>
          <button class="m3-btn" data-bool-op="intersect" ${multiDisabled}>Intersect</button>
          <button class="m3-btn" data-bool-op="subtract" ${multiDisabled}>Subtract</button>
          <button class="m3-btn" data-bool-op="trim" ${multiDisabled}>Trim</button>
          <button class="m3-btn" data-bool-op="join" ${multiDisabled}>Join</button>
          <button class="m3-btn" data-bool-op="offset" ${selDisabled}>Offset +0.18"</button>
          <button class="m3-btn ${this.showVertexGrips ? 'active' : ''}" id="btn-toggle-vertices" ${this.primarySelectedId ? '' : 'disabled'}>Edit Vertices</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Automation & Schema</span>
          <button class="m3-btn active" id="btn-open-vba-modal">VBA Studio...</button>
          <button class="m3-btn active" id="btn-open-sql-modal">SQL ⇄ ERD...</button>
        `;
    }
  }

  private renderSelectionOverlays(snap: EngineSnapshot, ppi: number): string {
    const selected2D = snap.shapes.filter(
      (s) => this.selectedShapeIds.includes(s.id) && s.kind !== 'Connector1D'
    );
    const boxesHtml = selected2D
      .map((sh, idx) => {
        const leftPx = (sh.pinX - sh.width * 0.5) * ppi;
        const topPx = (snap.pageHeightIn - sh.pinY - sh.height * 0.5) * ppi;
        const wPx = Math.max(14, sh.width * ppi);
        const hPx = Math.max(14, sh.height * ppi);
        const rotCss = sh.angleDeg ? `transform:rotate(${-sh.angleDeg}deg);` : '';

        if (idx > 0 || selected2D.length > 1) {
          return `<div class="selection-box-overlay multi" style="left:${leftPx}px;top:${topPx}px;width:${wPx}px;height:${hPx}px;${rotCss}"></div>`;
        }

        const ctrlDiamonds = (sh.controlHandles ?? [])
          .map((ch) => {
            const cxPx = ch.page_x_in * ppi;
            const cyPx = (snap.pageHeightIn - ch.page_y_in) * ppi;
            return `<div class="control-diamond-handle" data-ctrl-shape="${sh.id}" data-ctrl-row="${ch.name}" style="left:${cxPx.toFixed(1)}px;top:${cyPx.toFixed(1)}px" title="${ch.prompt || ch.name} (Drag Yellow Control Handle)"></div>`;
          })
          .join('');

        const vertexDots =
          this.showVertexGrips && this.primarySelectedId === sh.id
            ? this.activeShapeVertices
                .map((vx) => {
                  const vxPx = vx.page_x_in * ppi;
                  const vyPx = (snap.pageHeightIn - vx.page_y_in) * ppi;
                  return `<div class="vertex-edit-grip" data-vtx-shape="${sh.id}" data-vtx-geom="${vx.geom_index}" data-vtx-row="${vx.row_ix}" style="left:${vxPx.toFixed(1)}px;top:${vyPx.toFixed(1)}px" title="Geometry${vx.geom_index + 1}.Row_${vx.row_ix} (${vx.kind})"></div>`;
                })
                .join('')
            : '';

        // Primary selected shape gets 8 interactive resize handles + Top Rotation Knob + Yellow Control Diamonds + Vertex Grips!
        return `
          <div class="selection-box-overlay" style="left:${leftPx}px;top:${topPx}px;width:${wPx}px;height:${hPx}px;${rotCss}">
            <div class="rotation-stem"></div>
            <div class="rotation-knob" data-rotate-knob="${sh.id}" title="Drag to Rotate (${sh.angleDeg.toFixed(0)}°)"></div>
            <div class="selection-handle" data-handle="nw" style="left:-5px;top:-5px"></div>
            <div class="selection-handle" data-handle="n" style="left:calc(50% - 4.5px);top:-5px"></div>
            <div class="selection-handle" data-handle="ne" style="right:-5px;top:-5px"></div>
            <div class="selection-handle" data-handle="e" style="right:-5px;top:calc(50% - 4.5px)"></div>
            <div class="selection-handle" data-handle="se" style="right:-5px;bottom:-5px"></div>
            <div class="selection-handle" data-handle="s" style="left:calc(50% - 4.5px);bottom:-5px"></div>
            <div class="selection-handle" data-handle="sw" style="left:-5px;bottom:-5px"></div>
            <div class="selection-handle" data-handle="w" style="left:-5px;top:calc(50% - 4.5px)"></div>
          </div>
          ${ctrlDiamonds}
          ${vertexDots}
        `;
      })
      .join('');
    return boxesHtml;
  }

  public render(): void {
    const root = document.getElementById('app');
    if (!root || !this.snapshot) return;

    const snap = this.snapshot;
    const selectedShape =
      snap.shapes.find((s) => s.id === this.primarySelectedId) ?? snap.shapes[0] ?? null;
    const packNames = Object.keys(this.stencilCatalog);
    if (!packNames.includes(this.activeStencilPack) && packNames.length > 0) {
      this.activeStencilPack = packNames[0];
    }
    const masters = (this.stencilCatalog[this.activeStencilPack] ?? []).filter((m) =>
      m.toLowerCase().includes(this.stencilFilter.toLowerCase())
    );

    const ppi = 96;
    const sheetPxW = snap.pageWidthIn * ppi;
    const sheetPxH = snap.pageHeightIn * ppi;

    root.innerHTML = `
      <div class="workstation-shell">
        <header class="wco-titlebar">
          <span class="brand-pill no-drag" id="btn-brand-backstage" title="Open File Backstage & Templates">VEXIO</span>
          <div class="qat-group no-drag">
            <button class="m3-btn file-backstage-btn" id="btn-backstage">File / Templates</button>
            <button class="m3-btn" id="btn-open">Open Disk</button>
            <button class="m3-btn" id="btn-save">Save</button>
            <button class="m3-btn" id="btn-snapshot-quick" title="Save Immutable OPFS Version Checkpoint">+ Snapshot</button>
            <button class="m3-btn" id="btn-undo" ${snap.canUndo ? '' : 'disabled'}>Undo</button>
            <button class="m3-btn" id="btn-redo" ${snap.canRedo ? '' : 'disabled'}>Redo</button>
          </div>
          <span style="font-weight:600;margin-left:4px">${this.currentFileName}</span>
          <div style="flex:1"></div>
          <div class="peer-avatar-stack no-drag" id="peer-avatar-stack"></div>
          <span class="status-chip no-drag">
            ${snap.crossOriginIsolated ? 'SAB + SIMD' : 'WASM SIMD'} • ${this.wgpuAttached ? 'wgpu OffscreenCanvas' : snap.webgpuAvailable ? 'WebGPU Ready' : 'Vector GPU'}
          </span>
          <span class="status-chip ${this.cloudState.online ? '' : 'offline'} no-drag" id="cloud-sync-chip">
            ${this.cloudState.online ? 'Offline-Ready • OPFS Active' : '100% Offline Mode • OPFS WAL Active'}
          </span>
          <button class="m3-btn no-drag" id="btn-auth">
            ${this.cloudState.user ? 'Sign Out' : 'Cloud Sign-In'}
          </button>
        </header>

        <nav class="ribbon-bar">
          <div class="ribbon-tabs">
            ${['Home', 'Insert', 'Design', 'Data', 'Process', 'Review', 'View', 'Developer']
              .map(
                (tab) =>
                  `<button class="m3-btn ribbon-tab ${this.activeRibbonTab === tab ? 'active' : ''}" data-tab="${tab}">${tab}</button>`
              )
              .join('')}
          </div>
          <div class="ribbon-commands">
            ${this.renderContextualRibbon(snap)}
          </div>
        </nav>

        <main class="workspace-main">
          <aside class="stencil-pane">
            <div class="pane-header">12 Built-In Stencils (${packNames.length} .vssx Packs)</div>
            <select class="search-input" id="stencil-pack-select">
              ${packNames
                .map(
                  (pack) =>
                    `<option value="${pack}" ${pack === this.activeStencilPack ? 'selected' : ''}>${pack}</option>`
                )
                .join('')}
            </select>
            <input class="search-input" id="stencil-search" placeholder="Search masters..." value="${this.stencilFilter}" />
            <div class="stencil-grid">
              ${masters
                .map(
                  (m) =>
                    `<div class="stencil-item" draggable="true" data-master="${m}">
                      <div style="font-weight:600;margin-bottom:4px">${m}</div>
                      <div style="font-size:10px;color:var(--m3-text-muted)">Drag or Click</div>
                    </div>`
                )
                .join('')}
            </div>
          </aside>

          <section class="canvas-shell" style="${this.showRulers ? '' : 'grid-template-columns:0px 1fr;grid-template-rows:0px 1fr'}">
            <div class="ruler-corner">in</div>
            <div class="ruler-horiz">
              ${Array.from({ length: Math.ceil(snap.pageWidthIn) + 1 }, (_, i) => `<span style="width:${ppi * (this.zoomPct / 100)}px;flex-shrink:0">| ${i}"</span>`).join('')}
            </div>
            <div class="ruler-vert">
              ${Array.from({ length: Math.ceil(snap.pageHeightIn) + 1 }, (_, i) => `<span style="height:${ppi * (this.zoomPct / 100)}px;flex-shrink:0">${Math.ceil(snap.pageHeightIn) - i}"</span>`).join('')}
            </div>
            <div class="canvas-viewport">
              <div
                id="diagram-host"
                class="diagram-sheet-host tool-${this.activeTool}"
                style="width:${sheetPxW}px;height:${sheetPxH}px;transform:scale(${this.zoomPct / 100})"
              >
                <div id="wgpu-canvas-mount" class="wgpu-canvas-mount" style="${this.wgpuAttached ? 'opacity:0.22' : 'display:none'}"></div>
                <div class="svg-vector-layer">${snap.svgPreview}</div>
                ${this.renderSelectionOverlays(snap, ppi)}
                <div id="peer-cursor-layer" class="peer-cursor-layer"></div>
                <svg id="gesture-overlay-svg" class="live-overlay-svg" viewBox="0 0 ${sheetPxW} ${sheetPxH}"></svg>
              </div>
              ${
                this.showMinimap
                  ? `<div class="minimap-dock" title="Pan & Zoom Live Overview">
                      <div class="minimap-header">
                        <span>Pan & Zoom</span>
                        <span>${this.zoomPct}%</span>
                      </div>
                      <div class="minimap-stage" id="minimap-stage">
                        ${snap.svgPreview}
                        <div class="minimap-viewport-rect"></div>
                      </div>
                    </div>`
                  : ''
              }
            </div>
          </section>

          <aside class="inspector-pane">
            <div style="display:flex;gap:4px;flex-wrap:wrap">
              <button class="m3-btn ${this.activeInspectorTab === 'props' ? 'active' : ''}" data-insp="props">Inspector</button>
              <button class="m3-btn ${this.activeInspectorTab === 'layers' ? 'active' : ''}" data-insp="layers">Layers (${snap.layers.length})</button>
              <button class="m3-btn ${this.activeInspectorTab === 'comments' ? 'active' : ''}" data-insp="comments">Comments (${snap.comments.length})</button>
              <button class="m3-btn ${this.activeInspectorTab === 'validate' ? 'active' : ''}" data-insp="validate">Issues (${snap.validationIssues.length})</button>
            </div>

            ${this.renderInspectorBody(snap, selectedShape)}
          </aside>
        </main>

        <section class="shapesheet-dock ${this.showShapeSheet ? '' : 'hidden'}">
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px">
            <span class="pane-header">ShapeSheet Reactive Formula Editor — ${selectedShape ? `${selectedShape.name} (Sheet.${selectedShape.id})` : 'PageSheet'}</span>
            <div style="display:flex;gap:6px;align-items:center">
              <input class="cell-input" id="new-cell-name" placeholder="Cell (Width, Prop.Cost)" style="width:165px" value="Width" />
              <input class="cell-input" id="new-cell-formula" placeholder="Formula (=GUARD(3 in))" style="width:230px" value="=2.4 in" />
              <label style="font-size:11px"><input type="checkbox" id="force-setf-chk" /> SETF Override</label>
              <button class="m3-btn" id="btn-apply-formula">Commit Formula</button>
            </div>
          </div>
          <table class="ss-table">
            <thead><tr><th>Cell Name</th><th>Formula</th><th>Evaluated Value</th><th>Protection</th></tr></thead>
            <tbody>
              ${(selectedShape?.cells ?? [])
                .map(
                  (c) =>
                    `<tr>
                      <td>${c.name}</td>
                      <td>${c.formula}</td>
                      <td>${c.value}</td>
                      <td>${c.guarded ? 'GUARDED' : 'Editable'}</td>
                    </tr>`
                )
                .join('')}
            </tbody>
          </table>
        </section>

        <footer class="status-bar">
          <div class="page-tabs">
            ${snap.pages
              .map(
                (p) =>
                  `<button class="m3-btn page-tab-btn ${p.id === snap.activePageId ? 'active' : ''}" data-page="${p.id}">
                    ${p.name} (${p.shapeCount})
                  </button>`
              )
              .join('')}
            <button class="m3-btn" id="btn-add-page">+ Insert Page</button>
          </div>
          <div style="display:flex;align-items:center;gap:14px">
            <span id="coord-readout">X: ${this.cursorCoordsIn[0].toFixed(2)} in, Y: ${this.cursorCoordsIn[1].toFixed(2)} in</span>
            <span>Selected: <strong>${this.selectedShapeIds.length}</strong></span>
            <span>Peers: <strong>${this.peers.length + 1}</strong></span>
            <span>Fonts: ${this.systemFonts.length || 6}</span>
            <label>Zoom: ${this.zoomPct}% <input type="range" id="zoom-slider" min="45" max="200" value="${this.zoomPct}" /></label>
          </div>
        </footer>

        ${this.renderStudioModal()}
      </div>
    `;

    // Re-attach persistent OffscreenCanvas DOM node without losing WebGPU context
    const mount = document.getElementById('wgpu-canvas-mount');
    if (mount && !mount.contains(this.wgpuCanvasEl)) {
      mount.appendChild(this.wgpuCanvasEl);
    }
    this.renderPeersOverlayOnly();
    this.renderStatusChipsOnly();
    this.attachDomListeners();
  }

  private renderInspectorBody(snap: EngineSnapshot, selectedShape: EngineSnapshot['shapes'][number] | null): string {
    if (this.activeInspectorTab === 'layers') {
      return `
        <div class="pane-header">Visio & CAD Layer Manager</div>
        ${snap.layers
          .map(
            (l) => `
          <div style="padding:8px;border-radius:10px;background:var(--m3-surface-2);display:flex;align-items:center;justify-content:space-between">
            <strong>${l.name}</strong>
            <div style="display:flex;gap:8px;font-size:11px">
              <label><input type="checkbox" class="layer-vis-chk" data-ix="${l.ix}" ${l.visible ? 'checked' : ''} /> Visible</label>
              <label><input type="checkbox" class="layer-lock-chk" data-ix="${l.ix}" ${l.lock ? 'checked' : ''} /> Lock</label>
            </div>
          </div>`
          )
          .join('')}
      `;
    }

    if (this.activeInspectorTab === 'comments') {
      return `
        <div class="pane-header">Collaboration Review Comments</div>
        <div style="display:flex;gap:6px">
          <input class="cell-input" id="new-comment-input" placeholder="Add annotation on selected shape..." />
          <button class="m3-btn" id="btn-post-comment">Post</button>
        </div>
        ${
          snap.comments.length === 0
            ? '<div style="color:var(--m3-text-muted)">No comments on this page yet.</div>'
            : snap.comments
                .map(
                  (c) => `
              <div style="padding:8px;border-radius:10px;background:var(--m3-surface-2)">
                <div style="font-weight:600;color:var(--m3-primary)">${c.author} ${c.shapeId ? `• Sheet.${c.shapeId}` : ''}</div>
                <div style="margin-top:3px">${c.text}</div>
              </div>`
                )
                .join('')
        }
      `;
    }

    if (this.activeInspectorTab === 'validate') {
      return `
        <div class="pane-header">Validation Diagnostics (${snap.validationIssues.length})</div>
        ${
          snap.validationIssues.length === 0
            ? '<div style="color:var(--m3-secondary)">0 structural rule violations found.</div>'
            : snap.validationIssues
                .map(
                  (iss) => `
              <div style="padding:7px 9px;border-radius:8px;background:var(--m3-surface-2);border-left:3px solid var(--m3-error)">
                <strong>${iss.code}</strong> (${iss.severity}): ${iss.description}
              </div>`
                )
                .join('')
        }
      `;
    }

    return `
      <div class="pane-header">Shape Transform & Shape Data (${this.selectedShapeIds.length} selected)</div>
      <select class="search-input" id="shape-selector">
        ${snap.shapes
          .map(
            (s) =>
              `<option value="${s.id}" ${selectedShape?.id === s.id ? 'selected' : ''}>#${s.id} ${s.name} (${s.kind})</option>`
          )
          .join('')}
      </select>
      ${
        selectedShape
          ? `
        <label class="pane-header">Shape Text (**bold**, *italic*, [color=#2563EB]...[/color])</label>
        <input class="cell-input" id="shape-text-input" value="${selectedShape.text.replace(/"/g, '&quot;')}" />
        <div style="display:grid;grid-template-columns:1fr 1fr;gap:6px">
          <div><small>PinX (in)</small><input class="cell-input quick-dim" data-cell="PinX" value="${selectedShape.pinX.toFixed(2)} in" /></div>
          <div><small>PinY (in)</small><input class="cell-input quick-dim" data-cell="PinY" value="${selectedShape.pinY.toFixed(2)} in" /></div>
          <div><small>Width (in)</small><input class="cell-input quick-dim" data-cell="Width" value="${selectedShape.width.toFixed(2)} in" /></div>
          <div><small>Height (in)</small><input class="cell-input quick-dim" data-cell="Height" value="${selectedShape.height.toFixed(2)} in" /></div>
          <div style="grid-column:span 2"><small>Angle (deg)</small><input class="cell-input quick-dim" data-cell="Angle" value="${selectedShape.angleDeg.toFixed(1)} deg" /></div>
        </div>
        ${
          selectedShape.subprocessPage
            ? `<button class="m3-btn active" id="btn-jump-subprocess" data-sub-page="${selectedShape.subprocessPage}">[+] Open Subprocess Page: ${selectedShape.subprocessPage}</button>`
            : ''
        }
        ${
          selectedShape.screentip
            ? `<div style="font-size:11px;padding:6px 8px;border-radius:8px;background:var(--m3-surface-2);color:var(--m3-secondary)"><strong>ScreenTip:</strong> ${selectedShape.screentip}</div>`
            : ''
        }
      `
          : '<div>Click, drag, or marquee-lasso shapes on the page</div>'
      }
    `;
  }

  private renderStudioModal(): string {
    if (!this.activeModal) return '';

    if (this.activeModal === 'backstage') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong style="font-size:15px">Vexio File Backstage — Starter Templates, Workspaces & Version History</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>

            <div class="pane-header">8 Built-In Enterprise Starter Templates</div>
            <div class="backstage-grid">
              ${STARTER_TEMPLATES.map(
                (t) => `
                <div class="template-card" data-template-id="${t.id}">
                  <div style="font-size:10px;color:var(--m3-primary);font-weight:700">${t.category}</div>
                  <div style="font-weight:700">${t.title}</div>
                  <div style="font-size:11px;color:var(--m3-text-muted)">${t.desc}</div>
                </div>
              `
              ).join('')}
            </div>

            <div class="pane-header" style="margin-top:6px">Immutable Version History Snapshots (OPFS + Firebase Cloud)</div>
            <div style="display:flex;gap:8px">
              <input class="cell-input" id="version-label-input" value="${this.versionLabelDraft}" placeholder="Version label..." />
              <button class="m3-btn active" id="btn-create-version">Save Version Snapshot</button>
            </div>
            <div style="display:flex;flex-direction:column;gap:6px;max-height:145px;overflow-y:auto">
              ${
                this.opfsVersions.length === 0
                  ? '<div style="font-size:12px;color:var(--m3-text-muted)">No snapshots saved for this document yet.</div>'
                  : this.opfsVersions
                      .map(
                        (v) => `
                      <div class="list-row-card">
                        <div>
                          <strong>${v.label}</strong>
                          <span style="color:var(--m3-text-muted);font-size:11px;margin-left:8px">${v.createdAt} • ${(v.byteLength / 1024).toFixed(1)} KB</span>
                        </div>
                        <button class="m3-btn" data-restore-version="${v.versionId}">Restore</button>
                      </div>
                    `
                      )
                      .join('')
              }
            </div>

            <div class="pane-header" style="margin-top:6px">Local OPFS & Synced Cloud Workspaces</div>
            <div style="display:flex;gap:8px">
              <button class="m3-btn" id="btn-new-workspace">+ Create New Workspace Slot</button>
            </div>
            <div style="display:flex;flex-direction:column;gap:6px;max-height:140px;overflow-y:auto">
              ${this.opfsWorkspaces
                .map(
                  (w) => `
                <div class="list-row-card">
                  <div>
                    <strong>[OPFS] ${w.title}</strong>
                    <span style="color:var(--m3-text-muted);font-size:11px;margin-left:8px">${w.docId} • ${w.updatedAt}</span>
                  </div>
                  <button class="m3-btn" data-open-opfs-ws="${w.docId}" data-ws-title="${w.title}">Switch</button>
                </div>`
                )
                .join('')}
              ${this.cloudWorkspaces
                .map(
                  (cw) => `
                <div class="list-row-card">
                  <div>
                    <strong>[Firebase Cloud] ${cw.title}</strong>
                    <span style="color:var(--m3-secondary);font-size:11px;margin-left:8px">${cw.docId} • ${cw.updatedAt}</span>
                  </div>
                  <button class="m3-btn" data-open-cloud-ws="${cw.docId}" data-ws-title="${cw.title}">Pull & Open</button>
                </div>`
                )
                .join('')}
            </div>

            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'dataviz') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Visio Plan 2 Two-Way Data Visualizer (Excel / CSV ⇄ Cross-Functional Flowchart)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="font-size:12px;color:var(--m3-text-muted)">
              Bi-directional sync between <code>Process Step ID, Process Step Description, Next Step ID, Connector Label, Shape Type, Function, Owner</code> tables and auto-routed Swimlane diagrams.
            </div>
            <textarea class="studio-textarea" id="dataviz-editor">${this.dataVizDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px;flex-wrap:wrap">
              <button class="m3-btn" id="btn-pick-dataviz-xlsx">Import .xlsx / .csv File...</button>
              <button class="m3-btn" id="btn-export-dataviz-xlsx">Export Page → Data Visualizer (.xlsx)</button>
              <button class="m3-btn active" id="btn-import-dataviz-csv">Build Swimlane Flowchart from Table</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'cloud') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Cloud Architecture Auto-Discovery (AWS / Azure / GCP Inventory JSON)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="font-size:12px;color:var(--m3-text-muted)">
              Generates VPC/VNet boundary containers, compute/database/serverless/CDN topology nodes with <code>Prop.*</code> CIDR/provider metadata, and routed TLS/VPC connectors.
            </div>
            <textarea class="studio-textarea" id="cloud-json-editor">${this.cloudJsonDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn active" id="btn-build-cloud-topology">Generate Cloud Topology Page</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'compare') {
      const rep = this.latestDiffReport;
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Diagram Compare — Visual & ShapeSheet Structural Diff</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="display:flex;gap:8px;flex-wrap:wrap">
              <button class="m3-btn active" id="btn-compare-latest-opfs">Compare vs Latest OPFS Version Snapshot</button>
              <button class="m3-btn" id="btn-compare-disk-vsdx">Compare vs Local .vsdx File...</button>
            </div>
            ${
              rep
                ? `
                <div class="diff-summary-grid" style="grid-template-columns:repeat(3,1fr)">
                  <div class="diff-stat-pill"><strong style="color:#16a34a">+${rep.added_count}</strong><div>Added Shapes</div></div>
                  <div class="diff-stat-pill"><strong style="color:#dc2626">-${rep.removed_count}</strong><div>Removed Shapes</div></div>
                  <div class="diff-stat-pill"><strong style="color:#f59e0b">${rep.modified_count}</strong><div>Modified / Formula Diffs</div></div>
                </div>
                <div class="diff-svg-box">${rep.diff_overlay_svg}</div>
                <div style="max-height:135px;overflow-y:auto;display:flex;flex-direction:column;gap:4px;font-size:11.5px">
                  ${rep.entries
                    .map(
                      (e) =>
                        `<div class="list-row-card"><span><strong>${e.page_name} • ${e.shape_name} (#${e.shape_id})</strong></span><span style="color:var(--m3-text-muted)">${e.summary}</span></div>`
                    )
                    .join('')}
                </div>
              `
                : '<div style="color:var(--m3-text-muted);font-size:12px">Save a snapshot (+ Snapshot in titlebar) and edit shapes, or pick a baseline .vsdx file to inspect visual red/green/amber overlays and cell-level diffs.</div>'
            }
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'link') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Multi-Target Hyperlink & Hover ScreenTip</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <label class="pane-header">External URL Address</label>
            <input class="cell-input" id="link-addr-input" value="${this.linkAddressDraft}" placeholder="https://..." />
            <label class="pane-header">Internal Page SubAddress (Optional)</label>
            <input class="cell-input" id="link-sub-input" value="${this.linkSubAddressDraft}" placeholder="Page-2" />
            <label class="pane-header">Hover ScreenTip Comment</label>
            <input class="cell-input" id="link-tip-input" value="${this.linkScreentipDraft}" />
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn active" id="btn-save-link-tip">Apply to Selected Shape</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'org') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Organization Chart Wizard (CSV, Microsoft Entra Graph JSON, or LDAP .ldif)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="font-size:12px;color:var(--m3-text-muted)">
              Paste <code>Name,Title,Manager,Department</code> CSV, Microsoft Graph <code>{"users":[...]}</code> JSON, or RFC 2849 <code>dn:/cn:/title:/manager:</code> LDIF directory export.
            </div>
            <textarea class="studio-textarea" id="org-csv-editor">${this.orgCsvDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn" id="btn-build-org-entra-ldif">Import as Entra JSON / LDIF</button>
              <button class="m3-btn active" id="btn-build-org-chart">Generate Org Chart from CSV</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'vba') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Sandboxed VBA Automation Studio (UserForms, Events, If/Else, Selection)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <textarea class="studio-textarea" id="vba-editor">${this.vbaDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn active" id="btn-exec-vba">Execute in Bounded Sandbox</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'sql') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Database ERD Reverse & Forward SQL Schema Engineering</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <textarea class="studio-textarea" id="sql-editor">${this.sqlDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn" id="btn-forward-sql">Forward-Engineer Page → SQL DDL</button>
              <button class="m3-btn active" id="btn-reverse-sql">Reverse-Engineer SQL → Crow's Foot ERD</button>
            </div>
          </div>
        </div>
      `;
    }

    return `
      <div class="modal-backdrop">
        <div class="modal-card">
          <div style="display:flex;justify-content:space-between;align-items:center">
            <strong>External Data Recordset Binding (CSV / Native SQLite 3 Binary) & Live Data Graphics</strong>
            <button class="m3-btn" id="btn-close-modal">Close</button>
          </div>
          <div style="font-size:12px;color:var(--m3-text-muted)">
            Matches <code>Label</code> against shape text on the active page, binds <code>Prop.*</code> Shape Data, applies Color-by-Value + Data Bars + Icon Sets + Text Callouts, and inserts a live Page Legend—or bind directly from an offline <code>.sqlite</code> / <code>.db</code> B-tree file!
          </div>
          <textarea class="studio-textarea" id="csv-editor">${this.csvDraft}</textarea>
          ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
          <div style="display:flex;justify-content:flex-end;gap:8px">
            <button class="m3-btn" id="btn-bind-sqlite-file">Bind Local SQLite (.sqlite / .db)...</button>
            <button class="m3-btn active" id="btn-apply-csv-dg">Bind CSV & Apply Data Graphics</button>
          </div>
        </div>
      </div>
    `;
  }

  private async saveNewVersionSnapshot(label: string): Promise<void> {
    const { bytes } = await this.callEngine('saveFormat', { ext: 'vsdx' });
    const versionId = `v-${Date.now().toString(36)}`;
    const opfsCopy = new Uint8Array(bytes);
    const vRes = await this.callOpfs(
      'saveVersion',
      { docId: this.docId, versionId, label, vsdxBytes: opfsCopy },
      [opfsCopy.buffer]
    );
    this.opfsVersions = vRes.versions ?? this.opfsVersions;
    if (this.cloudState.user) {
      await this.firebaseSync.backupVersionToCloud(this.docId, versionId, label, bytes);
    }
  }

  private attachDomListeners(): void {
    document.getElementById('btn-backstage')?.addEventListener('click', () => this.openBackstageModal());
    document.getElementById('btn-brand-backstage')?.addEventListener('click', () => this.openBackstageModal());
    document.getElementById('btn-open-versions')?.addEventListener('click', () => this.openBackstageModal());

    document.getElementById('btn-snapshot-quick')?.addEventListener('click', async () => {
      await this.saveNewVersionSnapshot(`Checkpoint ${new Date().toLocaleTimeString()}`);
      this.openBackstageModal();
    });

    document.getElementById('btn-open')?.addEventListener('click', () => this.handleOpenDisk());
    document.getElementById('btn-ins-cad')?.addEventListener('click', () => this.handleOpenDisk());
    document.getElementById('btn-save')?.addEventListener('click', () => this.handleSaveInPlace('vsdx'));
    document.getElementById('btn-export-pdf')?.addEventListener('click', () => this.handleSaveInPlace('pdf'));
    document.getElementById('btn-export-pptx')?.addEventListener('click', () => this.handleSaveInPlace('pptx'));
    document.getElementById('btn-export-docx')?.addEventListener('click', () => this.handleSaveInPlace('docx'));
    document.getElementById('btn-export-dxf')?.addEventListener('click', () => this.handleSaveInPlace('dxf'));
    document.getElementById('btn-export-vsd')?.addEventListener('click', () => this.handleSaveInPlace('vsd'));
    document.getElementById('btn-export-flow')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('saveFormat', { ext: 'powerautomate' });
      await saveLocalDiagramInPlace(null, 'Vexio-PowerAutomate-Definition.flow.json', bytes);
    });
    document.getElementById('btn-export-pbi')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('saveFormat', { ext: 'powerbi' });
      await saveLocalDiagramInPlace(null, 'Vexio-PowerBI-VisualMap.pbi.json', bytes);
    });

    document.getElementById('btn-copy-svg')?.addEventListener('click', async () => {
      if (this.snapshot) {
        await copyDiagramVectorToClipboard(this.snapshot.svgPreview, this.snapshot.title);
      }
    });

    document.getElementById('btn-undo')?.addEventListener('click', async () => {
      this.snapshot = (await this.callEngine('undo')).snapshot;
      this.persistMutationSideEffects();
      this.render();
    });
    document.getElementById('btn-redo')?.addEventListener('click', async () => {
      this.snapshot = (await this.callEngine('redo')).snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-auth')?.addEventListener('click', async () => {
      if (this.cloudState.user) {
        await this.firebaseSync.signOutUser();
      } else {
        await this.firebaseSync.signInGoogleOrEmulator();
      }
    });

    // Ribbon Tabs & Contextual Commands
    document.querySelectorAll('.ribbon-tab').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeRibbonTab = (btn as HTMLElement).dataset.tab ?? 'Home';
        this.render();
      });
    });

    document.querySelectorAll('[data-tool]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeTool = ((btn as HTMLElement).dataset.tool as ActiveTool) ?? 'pointer';
        this.render();
      });
    });

    document.querySelectorAll('[data-align]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length < 2) return;
        const mode = (btn as HTMLElement).dataset.align ?? 'left';
        const res = await this.callEngine('alignShapes', {
          shapeIds: this.selectedShapeIds,
          mode,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.querySelectorAll('[data-bool-op]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const op = (btn as HTMLElement).dataset.boolOp ?? 'union';
        if (op !== 'offset' && this.selectedShapeIds.length < 2) return;
        if (op === 'offset' && this.selectedShapeIds.length < 1) return;
        const res = await this.callEngine('applyBooleanOp', {
          shapeIds: this.selectedShapeIds,
          op,
          offsetIn: 0.18,
        });
        this.selectedShapeIds = res.createdIds ?? [];
        if (this.showVertexGrips && this.primarySelectedId) {
          await this.refreshActiveVertices(this.primarySelectedId);
        }
        this.persistMutationSideEffects();
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.getElementById('btn-toggle-vertices')?.addEventListener('click', async () => {
      this.showVertexGrips = !this.showVertexGrips;
      if (this.showVertexGrips && this.primarySelectedId) {
        await this.refreshActiveVertices(this.primarySelectedId);
      } else {
        this.activeShapeVertices = [];
      }
      this.render();
    });

    document.querySelectorAll('[data-insp]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeInspectorTab = ((btn as HTMLElement).dataset.insp as InspectorTab) ?? 'props';
        this.render();
      });
    });

    document.getElementById('btn-delete-sel')?.addEventListener('click', async () => {
      if (this.selectedShapeIds.length === 0) return;
      const res = await this.callEngine('deleteShape', { shapeIds: this.selectedShapeIds });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
      this.render();
    });

    document.querySelectorAll('[data-fill-rgb]').forEach((sw) => {
      sw.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const rgb = (sw as HTMLElement).dataset.fillRgb;
        let lastRes: any = null;
        for (const sid of this.selectedShapeIds) {
          lastRes = await this.callEngine('setCellFormula', {
            shapeId: sid,
            cellName: 'FillForegnd',
            formula: `=RGB(${rgb})`,
            forceSetf: false,
          });
        }
        if (lastRes) {
          this.persistMutationSideEffects(lastRes.wal);
          this.snapshot = lastRes.snapshot;
          this.render();
        }
      });
    });

    document.getElementById('theme-slug-select')?.addEventListener('change', (e) => {
      this.selectedThemeSlug = (e.target as HTMLSelectElement).value;
    });
    document.getElementById('theme-variant-select')?.addEventListener('change', (e) => {
      this.selectedVariantIx = Number((e.target as HTMLSelectElement).value);
    });
    document.getElementById('btn-apply-theme')?.addEventListener('click', async () => {
      const res = await this.callEngine('applyThemeVariant', {
        themeSlug: this.selectedThemeSlug,
        variantIndex: this.selectedVariantIx,
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.querySelectorAll('[data-vis-preset]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const preset = (btn as HTMLElement).dataset.visPreset ?? 'shadow';
        const res = await this.callEngine('applyShapeVisualPreset', {
          shapeIds: this.selectedShapeIds,
          preset,
        });
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects();
        this.render();
      });
    });

    document.getElementById('btn-iso-title-block')?.addEventListener('click', async () => {
      const res = await this.callEngine('applyTitleBlockBorder', {
        projectCode: 'VEX-ARCH-2026',
        author: this.cloudState.user?.email ?? 'Principal Systems Architect',
        revision: 'Rev 2.0',
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-ins-image')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('image/png,image/jpeg,image/webp,image/svg+xml');
      if (!picked) return;
      const mediaType = picked.ext === 'jpg' || picked.ext === 'jpeg' ? 'JPEG' : 'PNG';
      const res = await this.callEngine('insertCroppedImage', {
        mediaType,
        bytes: picked.bytes,
        xIn: 5.5,
        yIn: 4.25,
        cropIn: 0.08,
        brightness: 1.04,
        contrast: 1.06,
      });
      this.snapshot = res.snapshot;
      this.setSingleSelection(res.shapeId);
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-ins-ctrl-handle')?.addEventListener('click', async () => {
      if (!this.primarySelectedId) return;
      const res = await this.callEngine('ensureControlHandle', { shapeId: this.primarySelectedId });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-create-subprocess')?.addEventListener('click', async () => {
      if (!this.primarySelectedId) return;
      const res = await this.callEngine('createSubprocess', {
        shapeId: this.primarySelectedId,
        subName: '',
        navigate: true,
      });
      this.snapshot = res.snapshot;
      this.selectedShapeIds = [];
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-jump-subprocess')?.addEventListener('click', async (e) => {
      const subPage = (e.currentTarget as HTMLElement).dataset.subPage;
      if (!subPage) return;
      const res = await this.callEngine('selectPage', { pageName: subPage });
      this.snapshot = res.snapshot;
      this.selectedShapeIds = [];
      this.render();
    });

    document.getElementById('btn-ins-container')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertContainer', {
        title: 'Subsystem Container',
        memberIds: this.selectedShapeIds,
      });
      this.persistMutationSideEffects();
      this.snapshot = res.snapshot;
      this.render();
    });

    document.getElementById('btn-ins-callout')?.addEventListener('click', async () => {
      if (!this.primarySelectedId) return;
      const res = await this.callEngine('insertCallout', {
        targetShapeId: this.primarySelectedId,
        text: 'SLA < 50ms',
      });
      this.persistMutationSideEffects();
      this.snapshot = res.snapshot;
      this.render();
    });

    document.getElementById('btn-ins-swimlanes')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertSwimlanes', {
        lanes: 'Customer Portal;Order Service;Fulfillment Warehouse',
      });
      this.persistMutationSideEffects();
      this.snapshot = res.snapshot;
      this.render();
    });

    document.getElementById('btn-ins-room')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertRoom', { roomName: 'Main Data Hall' });
      this.persistMutationSideEffects();
      this.snapshot = res.snapshot;
      this.setSingleSelection(res.roomId);
      this.render();
    });

    document.getElementById('btn-ins-rack')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertRack');
      this.persistMutationSideEffects();
      this.snapshot = res.snapshot;
      this.setSingleSelection(res.rackId);
      this.render();
    });

    document.querySelectorAll('[data-layout]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const algorithm = (btn as HTMLElement).dataset.layout ?? 'tb';
        this.snapshot = (await this.callEngine('autoLayout', { algorithm })).snapshot;
        this.persistMutationSideEffects();
        this.render();
      });
    });

    document.querySelectorAll('[data-pagesize]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const [w, h, j] = ((btn as HTMLElement).dataset.pagesize ?? '11,8.5,0').split(',').map(Number);
        this.snapshot = (
          await this.callEngine('setPageDimensions', { widthIn: w, heightIn: h, jumpStyle: j })
        ).snapshot;
        if (this.wgpuAttached) {
          const dpi = window.devicePixelRatio || 1;
          await this.callEngine('resizeOffscreenCanvas', {
            width: Math.round(w * 96 * dpi),
            height: Math.round(h * 96 * dpi),
            dpi,
          });
        }
        this.persistMutationSideEffects();
        this.render();
      });
    });

    document.querySelectorAll('[data-validate]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const ruleset = (btn as HTMLElement).dataset.validate ?? 'bpmn';
        this.snapshot = (await this.callEngine('validate', { ruleset })).snapshot;
        this.activeInspectorTab = 'validate';
        this.render();
      });
    });

    document.getElementById('btn-gen-pert')?.addEventListener('click', async () => {
      await this.callEngine('addPage', { name: 'PERT-CriticalPath' });
      this.snapshot = (await this.callEngine('generatePert')).snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-gen-gantt')?.addEventListener('click', async () => {
      await this.callEngine('addPage', { name: 'Gantt-Schedule' });
      this.snapshot = (await this.callEngine('generateGanttDemo')).snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-gen-timeline')?.addEventListener('click', async () => {
      await this.callEngine('addPage', { name: 'Project-Timeline' });
      this.snapshot = (await this.callEngine('generateTimelineDemo')).snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-export-bom')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('exportBomXlsx');
      await saveLocalDiagramInPlace(null, 'Vexio-BOM-Inventory-Report.xlsx', bytes);
    });

    document.getElementById('btn-add-quick-comment')?.addEventListener('click', () => {
      this.activeInspectorTab = 'comments';
      this.render();
    });

    document.getElementById('btn-open-layers')?.addEventListener('click', () => {
      this.activeInspectorTab = 'layers';
      this.render();
    });

    document.getElementById('btn-toggle-ss')?.addEventListener('click', () => {
      this.showShapeSheet = !this.showShapeSheet;
      this.render();
    });

    document.getElementById('btn-toggle-minimap')?.addEventListener('click', () => {
      this.showMinimap = !this.showMinimap;
      this.render();
    });

    document.getElementById('btn-toggle-rulers')?.addEventListener('click', () => {
      this.showRulers = !this.showRulers;
      this.render();
    });

    document.getElementById('btn-fit-window')?.addEventListener('click', () => {
      this.zoomPct = 100;
      this.render();
    });

    document.getElementById('btn-recover-opfs')?.addEventListener('click', async () => {
      const res = await this.callOpfs('readWal', { docId: this.docId });
      if (res.bytes && res.bytes.byteLength > 12) {
        this.snapshot = (await this.callEngine('replayWal', { bytes: res.bytes })).snapshot;
        this.render();
      }
    });

    // Studio & Backstage Modals
    document.getElementById('btn-open-vba-modal')?.addEventListener('click', () => {
      this.activeModal = 'vba';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-sql-modal')?.addEventListener('click', () => {
      this.activeModal = 'sql';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-data-modal')?.addEventListener('click', () => {
      this.activeModal = 'data';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-dataviz-modal')?.addEventListener('click', () => {
      this.activeModal = 'dataviz';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-cloud-modal')?.addEventListener('click', () => {
      this.activeModal = 'cloud';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-compare-modal')?.addEventListener('click', async () => {
      this.activeModal = 'compare';
      this.modalOutputMessage = '';
      try {
        const vRes = await this.callOpfs('listVersions', { docId: this.docId });
        this.opfsVersions = vRes.versions ?? [];
      } catch {
        // Offline ignore
      }
      this.render();
    });
    document.getElementById('btn-open-link-modal')?.addEventListener('click', () => {
      this.activeModal = 'link';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-org-modal')?.addEventListener('click', () => {
      this.activeModal = 'org';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-close-modal')?.addEventListener('click', () => {
      this.activeModal = null;
      this.render();
    });

    // Backstage Template Cards, Version Snapshots & Workspaces
    document.querySelectorAll('[data-template-id]').forEach((card) => {
      card.addEventListener('click', async () => {
        const tid = (card as HTMLElement).dataset.templateId ?? 'blank';
        const tpl = STARTER_TEMPLATES.find((t) => t.id === tid);
        const res = await this.callEngine('loadStarterTemplate', { templateId: tid });
        this.currentFileName = `${(tpl?.title ?? 'Diagram').replace(/[^a-zA-Z0-9-]+/g, '-')}.vsdx`;
        this.snapshot = res.snapshot;
        this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
        this.activeModal = null;
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.getElementById('btn-create-version')?.addEventListener('click', async () => {
      const inp = document.getElementById('version-label-input') as HTMLInputElement | null;
      this.versionLabelDraft = inp?.value.trim() || 'Milestone Checkpoint';
      await this.saveNewVersionSnapshot(this.versionLabelDraft);
      this.modalOutputMessage = `Saved immutable snapshot "${this.versionLabelDraft}" to OPFS${this.cloudState.user ? ' + Firebase Cloud' : ''}.`;
      this.render();
    });

    document.querySelectorAll('[data-restore-version]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const versionId = (btn as HTMLElement).dataset.restoreVersion!;
        const loaded = await this.callOpfs('loadVersion', { docId: this.docId, versionId });
        if (loaded.bytes && loaded.bytes.byteLength > 0) {
          const res = await this.callEngine('openBytes', { bytes: loaded.bytes, ext: 'vsdx' });
          this.snapshot = res.snapshot;
          this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
          this.activeModal = null;
          this.persistMutationSideEffects();
          this.render();
        }
      });
    });

    document.getElementById('btn-new-workspace')?.addEventListener('click', async () => {
      const slug = `workspace-${Date.now().toString(36).slice(-5)}`;
      this.docId = slug;
      this.currentFileName = `${slug}.vsdx`;
      this.collabSession.setRoom(this.docId);
      const res = await this.callEngine('loadStarterTemplate', { templateId: 'blank' });
      this.snapshot = res.snapshot;
      this.setSingleSelection(null);
      this.activeModal = null;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.querySelectorAll('[data-open-opfs-ws]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const el = btn as HTMLElement;
        const targetId = el.dataset.openOpfsWs!;
        const loaded = await this.callOpfs('loadWorkspace', { docId: targetId });
        if (loaded.bytes && loaded.bytes.byteLength > 0) {
          this.docId = targetId;
          this.currentFileName = el.dataset.wsTitle ?? `${targetId}.vsdx`;
          this.collabSession.setRoom(this.docId);
          const res = await this.callEngine('openBytes', { bytes: loaded.bytes, ext: 'vsdx' });
          this.snapshot = res.snapshot;
          this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
          this.activeModal = null;
          this.render();
        }
      });
    });

    document.querySelectorAll('[data-open-cloud-ws]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const el = btn as HTMLElement;
        const targetId = el.dataset.openCloudWs!;
        const bytes = await this.firebaseSync.restoreWorkspaceFromCloud(targetId, 'vsdx');
        if (bytes && bytes.byteLength > 0) {
          this.docId = targetId;
          this.currentFileName = el.dataset.wsTitle ?? `${targetId}.vsdx`;
          this.collabSession.setRoom(this.docId);
          const res = await this.callEngine('openBytes', { bytes, ext: 'vsdx' });
          this.snapshot = res.snapshot;
          this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
          this.activeModal = null;
          this.render();
        }
      });
    });

    document.getElementById('btn-import-dataviz-csv')?.addEventListener('click', async () => {
      this.dataVizDraft = (document.getElementById('dataviz-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'DataVisualizer-Flow' });
      const bytes = new TextEncoder().encode(this.dataVizDraft);
      const res = await this.callEngine('importDataVisualizer', { bytes });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Generated ${res.count} swimlanes, process shapes & routed connectors from Data Visualizer table.`;
      this.render();
    });

    document.getElementById('btn-pick-dataviz-xlsx')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('.xlsx,.csv');
      if (!picked) return;
      await this.callEngine('addPage', { name: 'DataVisualizer-Workbook' });
      const res = await this.callEngine('importDataVisualizer', { bytes: picked.bytes });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Imported ${picked.name} (${res.count} shapes & swimlanes).`;
      this.render();
    });

    document.getElementById('btn-export-dataviz-xlsx')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('exportDataVisualizer');
      await saveLocalDiagramInPlace(null, 'Vexio-DataVisualizer-ProcessMap.xlsx', bytes);
      this.modalOutputMessage = 'Exported bi-directional Data Visualizer .xlsx workbook.';
      this.render();
    });

    document.getElementById('btn-build-cloud-topology')?.addEventListener('click', async () => {
      this.cloudJsonDraft = (document.getElementById('cloud-json-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'Cloud-Topology' });
      const res = await this.callEngine('importCloudDiscovery', { json: this.cloudJsonDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Generated ${res.count} VPC containers, cloud resources & TLS connectors.`;
      this.render();
    });

    document.getElementById('btn-compare-latest-opfs')?.addEventListener('click', async () => {
      const latest = this.opfsVersions[this.opfsVersions.length - 1];
      if (!latest) {
        this.modalOutputMessage = 'No OPFS snapshot saved yet—click "+ Snapshot" in the titlebar first, then edit shapes.';
        this.render();
        return;
      }
      const loaded = await this.callOpfs('loadVersion', { docId: this.docId, versionId: latest.versionId });
      if (loaded.bytes && loaded.bytes.byteLength > 0) {
        const res = await this.callEngine('compareWithVsdx', { bytes: loaded.bytes });
        this.latestDiffReport = res.report;
        this.modalOutputMessage = `Compared current workspace against snapshot "${latest.label}".`;
        this.render();
      }
    });

    document.getElementById('btn-compare-disk-vsdx')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('.vsdx');
      if (!picked) return;
      const res = await this.callEngine('compareWithVsdx', { bytes: picked.bytes });
      this.latestDiffReport = res.report;
      this.modalOutputMessage = `Compared current workspace against local file "${picked.name}".`;
      this.render();
    });

    document.getElementById('btn-save-link-tip')?.addEventListener('click', async () => {
      if (!this.primarySelectedId) return;
      this.linkAddressDraft = (document.getElementById('link-addr-input') as HTMLInputElement).value;
      this.linkSubAddressDraft = (document.getElementById('link-sub-input') as HTMLInputElement).value;
      this.linkScreentipDraft = (document.getElementById('link-tip-input') as HTMLInputElement).value;
      const res = await this.callEngine('setHyperlinkAndScreentip', {
        shapeId: this.primarySelectedId,
        description: 'Architecture Link',
        address: this.linkAddressDraft,
        subAddress: this.linkSubAddressDraft,
        screentip: this.linkScreentipDraft,
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.activeModal = null;
      this.render();
    });

    document.getElementById('btn-build-org-chart')?.addEventListener('click', async () => {
      this.orgCsvDraft = (document.getElementById('org-csv-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'Org-Chart' });
      const res = await this.callEngine('importOrgChartCsv', { csv: this.orgCsvDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Generated ${res.count} executive/team Org Chart cards with reporting hierarchy.`;
      this.render();
    });

    document.getElementById('btn-build-org-entra-ldif')?.addEventListener('click', async () => {
      this.orgCsvDraft = (document.getElementById('org-csv-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'Directory-Org' });
      const res = await this.callEngine('importEntraOrLdif', { text: this.orgCsvDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Imported ${res.count} directory entries from Entra Graph JSON / LDIF.`;
      this.render();
    });

    document.getElementById('btn-exec-vba')?.addEventListener('click', async () => {
      this.vbaDraft = (document.getElementById('vba-editor') as HTMLTextAreaElement).value;
      const res = await this.callEngine('runMacro', {
        script: this.vbaDraft,
        selectedIds: this.selectedShapeIds,
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Executed ${res.macroReport.instructions_executed} sandboxed instructions (${res.macroReport.shapes_created} shapes created).`;
      this.render();
    });

    document.getElementById('btn-reverse-sql')?.addEventListener('click', async () => {
      this.sqlDraft = (document.getElementById('sql-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'ERD-Schema' });
      const res = await this.callEngine('reverseSql', { sql: this.sqlDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Reverse-engineered ${res.count} SQL tables into Crow's Foot ERD shapes & FK connectors.`;
      this.render();
    });

    document.getElementById('btn-forward-sql')?.addEventListener('click', async () => {
      const res = await this.callEngine('forwardSql');
      if (res.sql && res.sql.trim().length > 0) {
        this.sqlDraft = res.sql;
        this.modalOutputMessage = 'Generated SQL CREATE TABLE DDL from active ERD page.';
      } else {
        this.modalOutputMessage = 'No ERD entity tables on current page yet—click Reverse-Engineer first.';
      }
      this.render();
    });

    document.getElementById('btn-apply-csv-dg')?.addEventListener('click', async () => {
      this.csvDraft = (document.getElementById('csv-editor') as HTMLTextAreaElement).value;
      const res = await this.callEngine('linkCsvDataGraphics', {
        csv: this.csvDraft,
        keyCol: 'Label',
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Linked ${res.linked} shapes to External Data & applied Data Graphics + Page Legend!`;
      this.render();
    });

    document.getElementById('btn-bind-sqlite-file')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('.sqlite,.db,.sqlite3');
      if (!picked) return;
      const res = await this.callEngine('importSqliteRecordset', {
        bytes: picked.bytes,
        tableName: '',
        keyCol: 'Label',
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects();
      this.modalOutputMessage = `Bound ${res.count} shapes directly from offline SQLite B-tree database "${picked.name}".`;
      this.render();
    });

    // Stencils: Click OR Drag-and-Drop onto exact Canvas coordinates!
    document.getElementById('stencil-pack-select')?.addEventListener('change', (e) => {
      this.activeStencilPack = (e.target as HTMLSelectElement).value;
      this.render();
    });
    document.getElementById('stencil-search')?.addEventListener('input', (e) => {
      this.stencilFilter = (e.target as HTMLInputElement).value;
      this.render();
    });
    document.querySelectorAll('.stencil-item').forEach((item) => {
      const el = item as HTMLElement;
      const masterName = el.dataset.master ?? 'Process';
      el.addEventListener('dragstart', (ev) => {
        ev.dataTransfer?.setData(
          'application/x-vexio-stencil',
          JSON.stringify({ category: this.activeStencilPack, masterName })
        );
      });
      el.addEventListener('click', async () => {
        const count = this.snapshot?.shapes.length ?? 0;
        const res = await this.callEngine('dropShape', {
          category: this.activeStencilPack,
          masterName,
          xIn: 2.2 + (count % 4) * 2.1,
          yIn: 6.5 - Math.floor(count / 4) * 1.6,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.setSingleSelection(res.shapeId);
        this.render();
      });
    });

    // Direct-Manipulation Canvas Pointer, Yellow Control Diamonds, Vertex Grips, 8-Handle Resize, Rotation Knob, Lasso & USI Ink
    const host = document.getElementById('diagram-host');
    if (host) {
      host.addEventListener('dragover', (ev) => ev.preventDefault());
      host.addEventListener('drop', async (ev) => {
        ev.preventDefault();
        const raw = ev.dataTransfer?.getData('application/x-vexio-stencil');
        if (!raw) return;
        const { category, masterName } = JSON.parse(raw);
        const [xIn, yIn] = this.clientToPageInches(host, ev.clientX, ev.clientY);
        const res = await this.callEngine('dropShape', { category, masterName, xIn, yIn });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.setSingleSelection(res.shapeId);
        this.render();
      });

      host.addEventListener('dblclick', async (ev) => {
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);
        const { hitId } = await this.callEngine('hitTest', { xIn: ptIn[0], yIn: ptIn[1] });
        if (hitId > 0 && this.snapshot) {
          const sh = this.snapshot.shapes.find((s) => s.id === hitId);
          if (sh?.subprocessPage) {
            const res = await this.callEngine('selectPage', { pageName: sh.subprocessPage });
            this.snapshot = res.snapshot;
            this.selectedShapeIds = [];
            this.render();
          }
        }
      });

      host.addEventListener('pointerdown', async (ev) => {
        if (ev.button !== 0) return;
        const targetEl = ev.target as HTMLElement | null;
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);

        // 0A. Check Draggable Yellow Control Handle Diamond
        const ctrlShapeAttr = targetEl?.dataset.ctrlShape;
        if (ctrlShapeAttr) {
          ev.stopPropagation();
          host.setPointerCapture(ev.pointerId);
          this.dragState = {
            mode: 'control',
            startPageIn: ptIn,
            curPageIn: ptIn,
            shapeId: Number(ctrlShapeAttr),
            controlRowName: targetEl?.dataset.ctrlRow ?? 'Row_1',
          };
          return;
        }

        // 0B. Check Vertex / NURBS Edit Grip
        const vtxShapeAttr = targetEl?.dataset.vtxShape;
        if (vtxShapeAttr) {
          ev.stopPropagation();
          host.setPointerCapture(ev.pointerId);
          this.dragState = {
            mode: 'vertex',
            startPageIn: ptIn,
            curPageIn: ptIn,
            shapeId: Number(vtxShapeAttr),
            geomIndex: Number(targetEl?.dataset.vtxGeom ?? 0),
            vertexRowIx: Number(targetEl?.dataset.vtxRow ?? 1),
          };
          return;
        }

        // 1. Check Top Rotation Knob
        const rotIdAttr = targetEl?.dataset.rotateKnob;
        if (rotIdAttr && this.snapshot) {
          const sid = Number(rotIdAttr);
          const sh = this.snapshot.shapes.find((s) => s.id === sid);
          if (sh) {
            ev.stopPropagation();
            host.setPointerCapture(ev.pointerId);
            this.dragState = {
              mode: 'rotate',
              startPageIn: ptIn,
              curPageIn: ptIn,
              shapeId: sid,
              origPin: [sh.pinX, sh.pinY],
              origSize: [sh.width, sh.height],
              origAngleDeg: sh.angleDeg,
            };
            return;
          }
        }

        // 2. Check 8-Handle Resize Grips
        const handleDir = targetEl?.dataset.handle as ResizeHandleDir | undefined;
        if (handleDir && this.primarySelectedId && this.snapshot) {
          const sh = this.snapshot.shapes.find((s) => s.id === this.primarySelectedId);
          if (sh) {
            ev.stopPropagation();
            host.setPointerCapture(ev.pointerId);
            this.dragState = {
              mode: 'resize',
              startPageIn: ptIn,
              curPageIn: ptIn,
              shapeId: sh.id,
              origPin: [sh.pinX, sh.pinY],
              origSize: [sh.width, sh.height],
              origAngleDeg: sh.angleDeg,
              resizeHandle: handleDir,
            };
            return;
          }
        }

        host.setPointerCapture(ev.pointerId);

        if (this.activeTool === 'pen' || this.activeTool === 'highlighter') {
          this.dragState = {
            mode: 'ink',
            startPageIn: ptIn,
            curPageIn: ptIn,
            inkPoints: [
              {
                x: ptIn[0],
                y: ptIn[1],
                pressure: ev.pressure || 0.5,
                tilt_x: ev.tiltX || 0,
                tilt_y: ev.tiltY || 0,
              },
            ],
          };
          return;
        }

        if (this.activeTool === 'freeform') {
          this.dragState = {
            mode: 'freeform',
            startPageIn: ptIn,
            curPageIn: ptIn,
            freeformPoints: [ptIn],
          };
          return;
        }

        if (this.activeTool === 'rect') {
          this.dragState = { mode: 'rect', startPageIn: ptIn, curPageIn: ptIn };
          return;
        }

        const { hitId } = await this.callEngine('hitTest', { xIn: ptIn[0], yIn: ptIn[1] });
        if (this.activeTool === 'connector') {
          if (hitId > 0) {
            this.dragState = {
              mode: 'connect',
              startPageIn: ptIn,
              curPageIn: ptIn,
              shapeId: hitId,
            };
          }
          return;
        }

        if (hitId > 0) {
          if (ev.shiftKey) {
            if (this.selectedShapeIds.includes(hitId)) {
              this.selectedShapeIds = this.selectedShapeIds.filter((i) => i !== hitId);
            } else {
              this.selectedShapeIds = [...this.selectedShapeIds, hitId];
            }
            await this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds });
            this.render();
            return;
          }

          if (!this.selectedShapeIds.includes(hitId)) {
            this.setSingleSelection(hitId);
          }

          const sh = this.snapshot?.shapes.find((s) => s.id === hitId);
          if (sh && sh.kind !== 'Connector1D') {
            this.latestSmartGuides = [];
            this.latestSnappedPin = null;
            this.dragState = {
              mode: 'move',
              startPageIn: ptIn,
              curPageIn: ptIn,
              shapeId: hitId,
              origPin: [sh.pinX, sh.pinY],
              origSize: [sh.width, sh.height],
            };
          } else {
            this.render();
          }
        } else {
          // Empty canvas drag initiates Marquee Lasso Multi-Select!
          this.dragState = {
            mode: 'lasso',
            startPageIn: ptIn,
            curPageIn: ptIn,
          };
        }
      });

      host.addEventListener('pointermove', (ev) => {
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);
        this.cursorCoordsIn = ptIn;
        const coordEl = document.getElementById('coord-readout');
        if (coordEl) {
          coordEl.textContent = `X: ${ptIn[0].toFixed(2)} in, Y: ${ptIn[1].toFixed(2)} in`;
        }
        if (this.snapshot) {
          this.collabSession?.broadcastCursor(
            ptIn[0],
            ptIn[1],
            this.snapshot.activePageId,
            this.selectedShapeIds
          );
        }

        if (!this.dragState || !this.snapshot) return;
        this.dragState.curPageIn = ptIn;
        const overlay = document.getElementById('gesture-overlay-svg');
        if (!overlay) return;

        const ppi = 96;
        const pageH = this.snapshot.pageHeightIn;
        const sx = (x: number) => x * ppi;
        const sy = (y: number) => (pageH - y) * ppi;

        if (this.dragState.mode === 'ink' && this.dragState.inkPoints) {
          const coalesced = ev.getCoalescedEvents ? ev.getCoalescedEvents() : [ev];
          for (const ce of coalesced) {
            const p = this.clientToPageInches(host, ce.clientX, ce.clientY);
            this.dragState.inkPoints.push({
              x: p[0],
              y: p[1],
              pressure: ce.pressure || 0.5,
              tilt_x: ce.tiltX || 0,
              tilt_y: ce.tiltY || 0,
            });
          }
          const ptsAttr = this.dragState.inkPoints.map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join(' ');
          const strokeCol = this.activeTool === 'highlighter' ? '#fde047' : '#2563eb';
          const sw = this.activeTool === 'highlighter' ? 14 : 3;
          const op = this.activeTool === 'highlighter' ? 0.45 : 1.0;
          overlay.innerHTML = `<polyline points="${ptsAttr}" fill="none" stroke="${strokeCol}" stroke-width="${sw}" stroke-opacity="${op}" stroke-linecap="round"/>`;
        } else if (this.dragState.mode === 'freeform' && this.dragState.freeformPoints) {
          this.dragState.freeformPoints.push(ptIn);
          const ptsAttr = this.dragState.freeformPoints
            .map((p) => `${sx(p[0]).toFixed(1)},${sy(p[1]).toFixed(1)}`)
            .join(' ');
          overlay.innerHTML = `<polyline points="${ptsAttr}" fill="none" stroke="#7c3aed" stroke-width="2.5" stroke-dasharray="4,2" stroke-linecap="round"/>`;
        } else if (this.dragState.mode === 'control' || this.dragState.mode === 'vertex') {
          const fill = this.dragState.mode === 'control' ? '#facc15' : '#22d3ee';
          overlay.innerHTML = `<circle cx="${sx(ptIn[0]).toFixed(1)}" cy="${sy(ptIn[1]).toFixed(1)}" r="7" fill="${fill}" stroke="#111827" stroke-width="2"/>`;
        } else if (this.dragState.mode === 'rect' || this.dragState.mode === 'lasso') {
          const x0 = Math.min(sx(this.dragState.startPageIn[0]), sx(ptIn[0]));
          const y0 = Math.min(sy(this.dragState.startPageIn[1]), sy(ptIn[1]));
          const rw = Math.abs(sx(ptIn[0]) - sx(this.dragState.startPageIn[0]));
          const rh = Math.abs(sy(ptIn[1]) - sy(this.dragState.startPageIn[1]));
          const stroke = this.dragState.mode === 'lasso' ? '#0284c7' : '#2563eb';
          const fill = this.dragState.mode === 'lasso' ? 'rgba(2,132,199,0.10)' : 'rgba(37,99,235,0.12)';
          overlay.innerHTML = `<rect x="${x0}" y="${y0}" width="${rw}" height="${rh}" fill="${fill}" stroke="${stroke}" stroke-dasharray="5,4" stroke-width="1.8"/>`;
        } else if (this.dragState.mode === 'connect') {
          overlay.innerHTML = `<line x1="${sx(this.dragState.startPageIn[0])}" y1="${sy(this.dragState.startPageIn[1])}" x2="${sx(ptIn[0])}" y2="${sy(ptIn[1])}" stroke="#10b981" stroke-width="2.5" stroke-dasharray="6,4"/>`;
        } else if (this.dragState.mode === 'rotate' && this.dragState.origPin && this.dragState.origSize) {
          const [cx, cy] = this.dragState.origPin;
          const [w, h] = this.dragState.origSize;
          const rad = Math.atan2(ptIn[1] - cy, ptIn[0] - cx) - Math.PI * 0.5;
          const deg = (rad * 180) / Math.PI;
          overlay.innerHTML = `
            <g transform="rotate(${-deg.toFixed(1)} ${sx(cx).toFixed(1)} ${sy(cy).toFixed(1)})">
              <rect x="${sx(cx - w * 0.5)}" y="${sy(cy + h * 0.5)}" width="${w * ppi}" height="${h * ppi}" fill="rgba(29,78,216,0.10)" stroke="#1d4ed8" stroke-dasharray="4,3" stroke-width="2"/>
            </g>
          `;
        } else if (
          this.dragState.mode === 'resize' &&
          this.dragState.origPin &&
          this.dragState.origSize &&
          this.dragState.resizeHandle
        ) {
          const next = this.computeResizedBox(
            this.dragState.origPin,
            this.dragState.origSize,
            this.dragState.resizeHandle,
            ptIn[0] - this.dragState.startPageIn[0],
            ptIn[1] - this.dragState.startPageIn[1]
          );
          overlay.innerHTML = `<rect x="${sx(next.pinX - next.width * 0.5)}" y="${sy(next.pinY + next.height * 0.5)}" width="${next.width * ppi}" height="${next.height * ppi}" fill="rgba(37,99,235,0.10)" stroke="#2563eb" stroke-dasharray="4,3" stroke-width="2"/>`;
        } else if (this.dragState.mode === 'move' && this.dragState.origPin && this.dragState.shapeId) {
          const sh = this.snapshot.shapes.find((s) => s.id === this.dragState?.shapeId);
          if (sh) {
            const candX = this.dragState.origPin[0] + (ptIn[0] - this.dragState.startPageIn[0]);
            const candY = this.dragState.origPin[1] + (ptIn[1] - this.dragState.startPageIn[1]);

            if (!this.smartGuideReqInFlight && this.selectedShapeIds.length <= 1) {
              this.smartGuideReqInFlight = true;
              this.callEngine('computeSmartGuides', {
                shapeId: sh.id,
                candX,
                candY,
                width: sh.width,
                height: sh.height,
              })
                .then((gRes) => {
                  this.latestSmartGuides = gRes.guides ?? [];
                  this.latestSnappedPin = [gRes.snappedX ?? candX, gRes.snappedY ?? candY];
                })
                .finally(() => {
                  this.smartGuideReqInFlight = false;
                });
            }

            const drawX = this.latestSnappedPin ? this.latestSnappedPin[0] : candX;
            const drawY = this.latestSnappedPin ? this.latestSnappedPin[1] : candY;
            const guideSvg = this.latestSmartGuides
              .map((g) => {
                const col = g.kind === 'center' ? '#ec4899' : '#06b6d4';
                return g.axis === 'x'
                  ? `<line x1="${sx(g.posIn)}" y1="${sy(g.toIn)}" x2="${sx(g.posIn)}" y2="${sy(g.fromIn)}" stroke="${col}" stroke-width="1.6" stroke-dasharray="4,2"/>`
                  : `<line x1="${sx(g.fromIn)}" y1="${sy(g.posIn)}" x2="${sx(g.toIn)}" y2="${sy(g.posIn)}" stroke="${col}" stroke-width="1.6" stroke-dasharray="4,2"/>`;
              })
              .join('');

            overlay.innerHTML = `
              ${guideSvg}
              <rect x="${sx(drawX - sh.width * 0.5)}" y="${sy(drawY + sh.height * 0.5)}" width="${sh.width * ppi}" height="${sh.height * ppi}" fill="rgba(37,99,235,0.1)" stroke="#2563eb" stroke-dasharray="4,3" stroke-width="2"/>
            `;
          }
        }
      });

      host.addEventListener('pointerup', async (ev) => {
        if (!this.dragState) return;
        const st = this.dragState;
        this.dragState = null;
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);

        if (st.mode === 'ink' && st.inkPoints && st.inkPoints.length >= 2) {
          const isHi = this.activeTool === 'highlighter';
          const res = await this.callEngine('addInkStroke', {
            colorHex: isHi ? '#FACC15' : '#2563EB',
            widthPt: isHi ? 12 : 2.25,
            highlighter: isHi,
            points: st.inkPoints,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'freeform' && st.freeformPoints && st.freeformPoints.length >= 2) {
          const start = st.freeformPoints[0];
          const closed = Math.hypot(ptIn[0] - start[0], ptIn[1] - start[1]) < 0.28 && st.freeformPoints.length >= 5;
          const res = await this.callEngine('drawFreeformCurve', {
            points: st.freeformPoints,
            closed,
          });
          if (res.shapeId > 0) {
            this.snapshot = res.snapshot;
            this.setSingleSelection(res.shapeId);
            this.activeTool = 'pointer';
            this.persistMutationSideEffects();
          }
          this.render();
        } else if (st.mode === 'control' && st.shapeId) {
          const res = await this.callEngine('moveControlHandle', {
            shapeId: st.shapeId,
            rowName: st.controlRowName ?? 'Row_1',
            xIn: ptIn[0],
            yIn: ptIn[1],
          });
          this.snapshot = res.snapshot;
          this.persistMutationSideEffects();
          this.render();
        } else if (st.mode === 'vertex' && st.shapeId !== undefined) {
          const res = await this.callEngine('moveShapeVertex', {
            shapeId: st.shapeId,
            geomIndex: st.geomIndex ?? 0,
            rowIx: st.vertexRowIx ?? 1,
            xIn: ptIn[0],
            yIn: ptIn[1],
          });
          this.activeShapeVertices = res.vertices ?? [];
          this.snapshot = res.snapshot;
          this.persistMutationSideEffects();
          this.render();
        } else if (st.mode === 'lasso') {
          const dx = Math.abs(ptIn[0] - st.startPageIn[0]);
          const dy = Math.abs(ptIn[1] - st.startPageIn[1]);
          if (dx > 0.08 || dy > 0.08) {
            const qRes = await this.callEngine('queryBoxShapes', {
              x1: st.startPageIn[0],
              y1: st.startPageIn[1],
              x2: ptIn[0],
              y2: ptIn[1],
            });
            this.selectedShapeIds = qRes.ids ?? [];
          } else {
            this.selectedShapeIds = [];
            await this.callEngine('renderOffscreenFrame', { selectedIds: [] });
          }
          this.render();
        } else if (st.mode === 'rect') {
          const res = await this.callEngine('drawRect', {
            x1: st.startPageIn[0],
            y1: st.startPageIn[1],
            x2: ptIn[0],
            y2: ptIn[1],
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.setSingleSelection(res.shapeId);
          this.activeTool = 'pointer';
          this.render();
        } else if (st.mode === 'connect' && st.shapeId) {
          const { hitId } = await this.callEngine('hitTest', { xIn: ptIn[0], yIn: ptIn[1] });
          if (hitId > 0 && hitId !== st.shapeId) {
            const res = await this.callEngine('connectShapes', { fromId: st.shapeId, toId: hitId });
            this.persistMutationSideEffects(res.wal);
            this.snapshot = res.snapshot;
          }
          this.render();
        } else if (st.mode === 'rotate' && st.shapeId && st.origPin && st.origSize) {
          const [cx, cy] = st.origPin;
          const rad = Math.atan2(ptIn[1] - cy, ptIn[0] - cx) - Math.PI * 0.5;
          const angleDeg = Math.round(((rad * 180) / Math.PI) / 5) * 5;
          const res = await this.callEngine('transformShape', {
            shapeId: st.shapeId,
            pinX: cx,
            pinY: cy,
            width: st.origSize[0],
            height: st.origSize[1],
            angleDeg,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'resize' && st.shapeId && st.origPin && st.origSize && st.resizeHandle) {
          const next = this.computeResizedBox(
            st.origPin,
            st.origSize,
            st.resizeHandle,
            ptIn[0] - st.startPageIn[0],
            ptIn[1] - st.startPageIn[1]
          );
          const res = await this.callEngine('transformShape', {
            shapeId: st.shapeId,
            pinX: next.pinX,
            pinY: next.pinY,
            width: next.width,
            height: next.height,
            angleDeg: st.origAngleDeg ?? 0,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'move' && st.shapeId && st.origPin) {
          const dx = ptIn[0] - st.startPageIn[0];
          const dy = ptIn[1] - st.startPageIn[1];
          if (Math.hypot(dx, dy) > 0.04) {
            if (this.selectedShapeIds.length > 1 && this.selectedShapeIds.includes(st.shapeId)) {
              const res = await this.callEngine('moveShapesDelta', {
                shapeIds: this.selectedShapeIds,
                dxIn: dx,
                dyIn: dy,
              });
              this.persistMutationSideEffects(res.wal);
              this.snapshot = res.snapshot;
            } else {
              const finalX = this.latestSnappedPin ? this.latestSnappedPin[0] : st.origPin[0] + dx;
              const finalY = this.latestSnappedPin ? this.latestSnappedPin[1] : st.origPin[1] + dy;
              const res = await this.callEngine('moveShape', {
                shapeId: st.shapeId,
                xIn: finalX,
                yIn: finalY,
              });
              this.persistMutationSideEffects(res.wal);
              this.snapshot = res.snapshot;
            }
          }
          this.latestSmartGuides = [];
          this.latestSnappedPin = null;
          this.render();
        }
      });
    }

    // Inspector & Layer & Comment Listeners
    document.getElementById('shape-selector')?.addEventListener('change', async (e) => {
      this.setSingleSelection(Number((e.target as HTMLSelectElement).value));
      await this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds });
      this.render();
    });

    document.getElementById('shape-text-input')?.addEventListener('change', async (e) => {
      if (this.primarySelectedId === null) return;
      const res = await this.callEngine('setShapeText', {
        shapeId: this.primarySelectedId,
        text: (e.target as HTMLInputElement).value,
      });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.render();
    });

    document.querySelectorAll('.quick-dim').forEach((inp) => {
      inp.addEventListener('change', async (e) => {
        if (!this.primarySelectedId) return;
        const el = e.target as HTMLInputElement;
        const cellName = el.dataset.cell ?? 'Width';
        const res = await this.callEngine('setCellFormula', {
          shapeId: this.primarySelectedId,
          cellName,
          formula: el.value,
          forceSetf: false,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.querySelectorAll('.layer-vis-chk, .layer-lock-chk').forEach((chk) => {
      chk.addEventListener('change', async () => {
        const ix = Number((chk as HTMLElement).dataset.ix);
        const visEl = document.querySelector(`.layer-vis-chk[data-ix="${ix}"]`) as HTMLInputElement;
        const lockEl = document.querySelector(`.layer-lock-chk[data-ix="${ix}"]`) as HTMLInputElement;
        this.snapshot = (
          await this.callEngine('updateLayer', {
            ix,
            visible: visEl?.checked ?? true,
            lock: lockEl?.checked ?? false,
            colorHex: '',
          })
        ).snapshot;
        this.persistMutationSideEffects();
        this.render();
      });
    });

    document.getElementById('btn-post-comment')?.addEventListener('click', async () => {
      const inp = document.getElementById('new-comment-input') as HTMLInputElement | null;
      if (!inp || !inp.value.trim()) return;
      this.snapshot = (
        await this.callEngine('addComment', {
          shapeId: this.primarySelectedId ?? 0,
          author: this.cloudState.user?.email ?? this.collabSession.peerName,
          text: inp.value.trim(),
        })
      ).snapshot;
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('btn-apply-formula')?.addEventListener('click', async () => {
      const shapeId = this.primarySelectedId ?? this.snapshot?.shapes[0]?.id;
      if (!shapeId) return;
      const cellName = (document.getElementById('new-cell-name') as HTMLInputElement).value;
      const formula = (document.getElementById('new-cell-formula') as HTMLInputElement).value;
      const forceSetf = (document.getElementById('force-setf-chk') as HTMLInputElement).checked;
      const res = await this.callEngine('setCellFormula', { shapeId, cellName, formula, forceSetf });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.render();
    });

    document.querySelectorAll('.page-tab-btn').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const pageId = Number((btn as HTMLElement).dataset.page);
        this.snapshot = (await this.callEngine('selectPage', { pageId })).snapshot;
        this.selectedShapeIds = [];
        this.render();
      });
    });

    document.getElementById('btn-add-page')?.addEventListener('click', async () => {
      const nextNum = (this.snapshot?.pages.length ?? 1) + 1;
      this.snapshot = (await this.callEngine('addPage', { name: `Page-${nextNum}` })).snapshot;
      this.selectedShapeIds = [];
      this.persistMutationSideEffects();
      this.render();
    });

    document.getElementById('zoom-slider')?.addEventListener('input', (e) => {
      this.zoomPct = Number((e.target as HTMLInputElement).value);
      this.render();
    });
  }

  private computeResizedBox(
    origPin: [number, number],
    origSize: [number, number],
    dir: ResizeHandleDir,
    dx: number,
    dy: number
  ): { pinX: number; pinY: number; width: number; height: number } {
    let left = origPin[0] - origSize[0] * 0.5;
    let right = origPin[0] + origSize[0] * 0.5;
    let bottom = origPin[1] - origSize[1] * 0.5;
    let top = origPin[1] + origSize[1] * 0.5;

    if (dir.includes('w')) left = Math.min(right - 0.2, left + dx);
    if (dir.includes('e')) right = Math.max(left + 0.2, right + dx);
    if (dir.includes('s')) bottom = Math.min(top - 0.2, bottom + dy);
    if (dir.includes('n')) top = Math.max(bottom + 0.2, top + dy);

    return {
      pinX: (left + right) * 0.5,
      pinY: (bottom + top) * 0.5,
      width: Math.max(0.2, right - left),
      height: Math.max(0.2, top - bottom),
    };
  }
}

new VexioChromeOsApp().boot();
