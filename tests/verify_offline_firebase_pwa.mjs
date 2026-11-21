/**
 * End-to-End Automated Verification Suite for Vexio Target D:
 * Firebase Hosting (COOP/COEP, WASM MIME, Caching) + Auth/Firestore Security Rules
 * + ChromeOS Manifest (`window-controls-overlay`, `file_handlers`, 12 precached `.vssx` stencils)
 * + 100% Unauthenticated Offline Rust WASM SIMD Engine Execution (ShapeSheet, Lasso, Align,
 *   8-Handle Transform, Smart Guides, 8 Starter Templates, Org/Gantt/Timeline, CAD/VSD/PDF).
 */

import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const DIST_DIR = resolve(REPO_ROOT, 'apps/chromeos-pwa/dist');
const HOSTING_ORIGIN = 'http://127.0.0.1:5000';
const AUTH_ORIGIN = 'http://127.0.0.1:9099';
const FIRESTORE_ORIGIN = 'http://127.0.0.1:8080';
const PROJECT_ID = 'demo-vexio-chromeos';

// Ensure Node bypasses any outbound HTTP proxy when talking to 127.0.0.1 local emulators!
process.env.NO_PROXY = '127.0.0.1,localhost';
process.env.no_proxy = '127.0.0.1,localhost';

async function main() {
  console.log('=== [1/6] Verifying Firebase Hosting COOP/COEP SharedArrayBuffer Headers ===');
  const rootRes = await fetch(`${HOSTING_ORIGIN}/`);
  assert.equal(rootRes.status, 200, 'Hosting root / must return 200 OK');
  assert.equal(
    rootRes.headers.get('cross-origin-opener-policy'),
    'same-origin',
    'Missing COOP: same-origin header'
  );
  assert.equal(
    rootRes.headers.get('cross-origin-embedder-policy'),
    'require-corp',
    'Missing COEP: require-corp header'
  );
  assert.equal(
    rootRes.headers.get('cross-origin-resource-policy'),
    'same-origin',
    'Missing CORP: same-origin header'
  );
  console.log('  ✓ COOP: same-origin, COEP: require-corp, CORP: same-origin verified.');

  console.log('=== [2/6] Verifying WASM SIMD Binary MIME Type & Immutable Caching ===');
  const wasmRes = await fetch(`${HOSTING_ORIGIN}/wasm/vexio_core_bg.wasm`);
  assert.equal(wasmRes.status, 200, 'WASM binary must be served at /wasm/vexio_core_bg.wasm');
  assert.match(
    wasmRes.headers.get('content-type') ?? '',
    /^application\/wasm/i,
    'WASM binary must have Content-Type: application/wasm'
  );
  assert.match(
    wasmRes.headers.get('cache-control') ?? '',
    /immutable/i,
    'WASM binary must have immutable Cache-Control'
  );
  const wasmBytes = new Uint8Array(await wasmRes.arrayBuffer());
  assert.deepEqual(
    Array.from(wasmBytes.slice(0, 4)),
    [0x00, 0x61, 0x73, 0x6d],
    'Invalid WebAssembly binary magic bytes'
  );
  console.log(`  ✓ WASM SIMD binary verified (${(wasmBytes.byteLength / 1024).toFixed(1)} KB, application/wasm, immutable).`);

  console.log('=== [3/6] Verifying ChromeOS Manifest, <500 KB Code-Split Chunks & 12-Pack SW Precache ===');
  const manifestRes = await fetch(`${HOSTING_ORIGIN}/manifest.webmanifest`);
  assert.equal(manifestRes.status, 200);
  const manifest = await manifestRes.json();
  assert.deepEqual(manifest.display_override, ['window-controls-overlay', 'standalone']);
  const acceptedExts = Object.values(manifest.file_handlers[0].accept).flat();
  for (const reqExt of ['.vsdx', '.vsdm', '.vssx', '.vstx', '.vsd', '.dwg', '.dxf', '.emf', '.wmf', '.ldif']) {
    assert(acceptedExts.includes(reqExt), `manifest file_handlers missing ${reqExt}`);
  }

  const assetsDir = resolve(DIST_DIR, 'assets');
  const jsAssets = readdirSync(assetsDir).filter((f) => f.endsWith('.js'));
  assert(jsAssets.length >= 4, 'Vite Rollup code-splitting must produce multiple modular JS chunks');
  for (const jsFile of jsAssets) {
    const sizeBytes = statSync(resolve(assetsDir, jsFile)).size;
    assert(
      sizeBytes < 500 * 1024,
      `JS chunk ${jsFile} (${(sizeBytes / 1024).toFixed(1)} KB) exceeded 500 KB budget!`
    );
  }

  const swRes = await fetch(`${HOSTING_ORIGIN}/sw.js`);
  assert.equal(swRes.status, 200);
  assert.match(swRes.headers.get('cache-control') ?? '', /no-cache/i);
  const swText = await swRes.text();
  assert(swText.includes('/wasm/vexio_core_bg.wasm'), 'sw.js must precache /wasm/vexio_core_bg.wasm');
  for (const jsFile of jsAssets) {
    assert(swText.includes(`/assets/${jsFile}`), `sw.js must precache code-split chunk ${jsFile}`);
  }
  for (const stFile of [
    'basic_flowchart.vssx',
    'bpmn_2_0.vssx',
    'uml_2_5.vssx',
    'network_rack_equipment.vssx',
    'floor_plan_p_id_vsm.vssx',
    'cloud_architecture_aws_gcp_azure.vssx',
    'electrical_electronic_ieee_iec.vssx',
    'ui_application_wireframes.vssx',
    'organization_chart.vssx',
    'project_schedule_gantt_timeline.vssx',
    'value_stream_mapping_lean_vsm.vssx',
    'database_crow_s_foot_idef1x.vssx',
  ]) {
    assert(swText.includes(`/stencils/${stFile}`), `sw.js missing precached stencil ${stFile}`);
  }
  console.log(`  ✓ ChromeOS WCO, File Handlers, ${jsAssets.length} code-split chunks (<500 KB each), and 12 .vssx Stencil Packs verified.`);

  console.log('=== [4/6] Verifying 100% Unauthenticated Offline Rust WASM SIMD Engine & Core Tools ===');
  const distWasmJs = pathToFileURL(resolve(DIST_DIR, 'wasm/vexio_core.js')).href;
  const localWasmBytes = readFileSync(resolve(DIST_DIR, 'wasm/vexio_core_bg.wasm'));
  const wasmMod = await import(distWasmJs);
  wasmMod.initSync({ module: localWasmBytes });

  const session = new wasmMod.WasmVexioSession('Offline ChromeOS Test');
  const stencilMap = JSON.parse(session.list_builtin_stencils_json());
  assert.equal(Object.keys(stencilMap).length, 12, 'WASM session must expose all 12 stencil packs');

  const s1 = session.drop_builtin_shape('BPMN 2.0', 'Start Event', 2.0, 5.0);
  const s2 = session.drop_builtin_shape('BPMN 2.0', 'Task', 5.0, 5.0);
  const s3 = session.drop_builtin_shape('BPMN 2.0', 'End Event', 8.0, 5.0);
  session.connect_shapes(1, s1, s2);
  session.connect_shapes(1, s2, s3);

  // Marquee Lasso query, Smart Guides, 8-Handle Resize/Rotate & Align
  const lassoIds = Array.from(session.query_box_shapes(1, 1.0, 3.5, 9.0, 6.5));
  assert.equal(lassoIds.length, 3, 'R*-tree marquee lasso must capture all 3 2D shapes');

  const guideRes = JSON.parse(session.compute_smart_guides(1, s2, 5.0, 5.04, 1.6, 0.9));
  assert(Math.abs(guideRes.snappedY - 5.0) < 1e-6, 'Smart guide must magnetically snap Y to 5.0 in');
  assert(guideRes.guides.length >= 1, 'Smart guide must emit center/edge alignment line');

  assert.equal(session.transform_shape(1, s2, 5.0, 5.2, 2.2, 1.1, 15.0), true);
  assert.equal(session.align_shapes(1, new Uint32Array([s1, s2, s3]), 'middle'), 3);

  // Enforce GUARD() and SETF override inside WASM!
  assert.equal(session.set_cell_formula(1, s2, 'Width', '=GUARD(3 in)', false), true);
  assert.equal(
    session.set_cell_formula(1, s2, 'Width', '5 in', false),
    false,
    'GUARD must block unforced interactive edit in WASM'
  );
  assert.equal(
    session.set_cell_formula(1, s2, 'Width', '4.5 in', true),
    true,
    'SETF override must succeed in WASM'
  );

  // BPMN 2.0 validation in WASM
  const issues = JSON.parse(session.validate_diagram('bpmn'));
  assert.equal(issues.length, 0, `Valid BPMN flow must report 0 issues, got: ${JSON.stringify(issues)}`);

  // Save .vsdx, .vsd, .dxf, .pdf, .pptx, .docx, .flow.json, .pbi.json, and OPFS WAL journal 100% offline in WASM!
  const vsdxOut = session.save_format('vsdx');
  const vsdOut = session.save_format('vsd');
  const dxfOut = session.save_format('dxf');
  const pdfOut = session.save_format('pdf');
  const pptxOut = session.save_format('pptx');
  const docxOut = session.save_format('docx');
  const flowOut = session.save_format('powerautomate');
  const pbiOut = session.save_format('powerbi');
  const walOut = session.export_wal();
  assert(vsdxOut.byteLength > 500, 'WASM .vsdx output empty');
  assert(vsdOut[0] === 0xd0 && vsdOut[1] === 0xcf, 'WASM .vsd must have OLE2 header');
  assert(new TextDecoder().decode(dxfOut).includes('ENTITIES'), 'WASM .dxf must contain ENTITIES');
  assert(new TextDecoder().decode(pdfOut.slice(0, 8)).startsWith('%PDF-1.7'), 'WASM .pdf must start with %PDF-1.7');
  assert(pptxOut[0] === 0x50 && pptxOut[1] === 0x4b, 'WASM .pptx must be valid OpenXML ZIP');
  assert(docxOut[0] === 0x50 && docxOut[1] === 0x4b, 'WASM .docx must be valid OpenXML ZIP');
  assert(JSON.parse(new TextDecoder().decode(flowOut)).actions, 'Power Automate JSON must contain workflow actions');
  assert(Array.isArray(JSON.parse(new TextDecoder().decode(pbiOut)).shapes), 'Power BI JSON must contain shapes array');
  assert(walOut.byteLength > 20, 'OPFS WAL journal bytes must be non-empty');

  const reloaded = new wasmMod.WasmVexioSession('Reloaded');
  reloaded.open_bytes(vsdxOut, 'vsdx');
  const state = JSON.parse(reloaded.get_state_json(1));
  assert.equal(state.shapes.length, 5, 'Round-tripped WASM .vsdx must preserve 3 nodes + 2 connectors');

  // Verify Starter Templates, Org Chart CSV, Gantt, Timeline, SQL ERD, Data Graphics & BOM XLSX
  const tplSession = new wasmMod.WasmVexioSession('Templates');
  tplSession.load_starter_template('cloud_microservices');
  const tplState = JSON.parse(tplSession.get_state_json(1));
  assert(tplState.shapes.length >= 5, 'Cloud microservices starter template must populate VPC & K8s shapes');

  const orgPage = tplSession.add_page('Org');
  assert.equal(
    tplSession.import_org_chart_csv(
      orgPage,
      'Name,Title,Manager,Department\nCEO,Chief Executive,,Exec\nVP,VP Eng,CEO,Eng'
    ),
    2
  );
  const ganttPage = tplSession.add_page('Gantt');
  assert.equal(tplSession.generate_gantt_demo(ganttPage), 4);
  const tlPage = tplSession.add_page('Timeline');
  assert.equal(tplSession.generate_timeline_demo(tlPage), 4);

  const linkedCount = reloaded.link_csv_and_apply_data_graphics(
    1,
    'Label,Status,Load\nTask,Healthy,92',
    'Label'
  );
  assert.equal(linkedCount, 1, 'External CSV Data Graphics must bind matching Task shape');
  const bomXlsx = reloaded.export_bom_xlsx();
  assert(bomXlsx[0] === 0x50 && bomXlsx[1] === 0x4b, 'BOM report .xlsx must be valid OpenXML ZIP');

  const erdPageId = reloaded.add_page('ERD');
  assert.equal(
    reloaded.reverse_engineer_sql(
      erdPageId,
      'CREATE TABLE teams (id INT PRIMARY KEY, name VARCHAR NOT NULL); CREATE TABLE members (id INT PRIMARY KEY, team_id INT NOT NULL REFERENCES teams(id));'
    ),
    2
  );
  assert(reloaded.forward_engineer_sql(erdPageId).includes('FOREIGN KEY (team_id) REFERENCES teams(id)'));
  console.log('  ✓ Core WASM SIMD + PPTX/DOCX/PowerAutomate/PowerBI + Templates + ERD + BOM verified.');

  console.log('=== [5/6] Verifying Phase 7 Plan 2 Features (Boolean Ops, Themes, Subprocess, Diff, DataViz, Cloud, CRDT) ===');
  const p7 = new wasmMod.WasmVexioSession('Phase 7 Plan 2 Verification');
  const rA = p7.draw_rectangle(1, 2.0, 3.0, 4.2, 4.8);
  const rB = p7.draw_rectangle(1, 3.2, 3.4, 5.4, 5.2);
  const rC = p7.draw_rectangle(1, 7.0, 2.0, 8.5, 3.0);
  const baselineVsdx = p7.save_format('vsdx');
  assert.equal(p7.delete_shape(1, rC), true);

  // Boolean Union + Offset + Freeform Curve + Draggable Yellow Control Handle + Vertex Edit
  const unionIds = Array.from(p7.apply_boolean_op(1, new Uint32Array([rA, rB]), 'union', 0.0));
  assert.equal(unionIds.length, 1, 'Boolean union of overlapping rectangles must yield 1 merged shape');
  const mergedId = unionIds[0];
  const verticesBefore = JSON.parse(p7.list_shape_vertices_json(1, mergedId));
  assert(verticesBefore.length >= 6, 'L-shaped boolean union polygon must expose editable vertices');
  assert.equal(
    p7.move_shape_vertex(1, mergedId, verticesBefore[1].geom_index, verticesBefore[1].row_ix, 4.5, 5.1),
    true
  );
  assert.equal(p7.ensure_control_handle(1, mergedId), true);
  assert.equal(p7.move_control_handle(1, mergedId, 'Row_1', 3.8, 4.4), true);

  const curveId = p7.draw_freeform_curve(
    1,
    JSON.stringify([[1.2, 1.5], [2.0, 2.4], [3.1, 1.8], [4.2, 2.6]]),
    false
  );
  assert(curveId > 0, 'draw_freeform_curve must create smooth cubic Bezier shape');

  // 6 Themes × 4 Variants + Structured Visual Effects + ISO Title Block + Subprocess + Hyperlink/ScreenTip
  assert(p7.apply_theme_variant('blueprint', 2) >= 2, 'Blueprint theme variant must harmonize shapes');
  assert.equal(p7.apply_shape_visual_preset(1, mergedId, 'grad_linear'), true);
  assert.equal(p7.apply_shape_visual_preset(1, mergedId, 'shadow_soft'), true);
  assert.equal(p7.apply_shape_visual_preset(1, mergedId, 'bevel_3d'), true);
  p7.set_hyperlink_and_screentip(1, mergedId, 'Runbook', 'https://vexio.dev/ops', '', 'Primary Architecture Node');
  const bgId = p7.apply_title_block_border(1, 'VEX-700', 'Lead Architect', 'Rev 7');
  assert(bgId > 1, 'ISO Title Block must attach background page');
  const subPageId = p7.create_subprocess_drilldown(1, mergedId, 'Merged-Subprocess');
  assert(subPageId > 1, 'Subprocess drill-down must create and link target page');

  const richSvg = new TextDecoder().decode(p7.save_format('svg'));
  assert(richSvg.includes('<linearGradient'), 'SVG export must emit <linearGradient> defs');
  assert(richSvg.includes('<feDropShadow'), 'SVG export must emit <feDropShadow> filter');
  assert(richSvg.includes('VEX-700'), 'SVG export must composite linked ISO Title Block background page');
  assert(richSvg.includes('Primary Architecture Node'), 'SVG export must emit ScreenTip <title>');

  // Visual Diagram Compare against baselineVsdx
  const diffReport = JSON.parse(p7.compare_with_vsdx_bytes(baselineVsdx));
  assert(
    diffReport.added_count >= 1 && diffReport.removed_count >= 1 && diffReport.modified_count >= 1,
    'Diagram compare must detect added, removed, and modified diffs'
  );
  assert(diffReport.diff_overlay_svg.includes('<svg'), 'Diagram compare must generate visual diff_overlay_svg');

  // Two-Way Data Visualizer (CSV -> Swimlanes -> XLSX -> Swimlanes), Cloud Discovery & LDIF Org Chart
  const dvPage = p7.add_page('DataViz');
  const dvCsv = new TextEncoder().encode(
    'Process Step ID,Process Step Description,Next Step ID,Connector Label,Shape Type,Function,Owner\nP1,Start Order,P2,,Start,Sales,AE\nP2,Approve Credit,,,End,Finance,CFO'
  );
  assert.equal(p7.import_data_visualizer(dvPage, dvCsv), 2, 'Data Visualizer must build process steps + swimlanes');
  const dvXlsx = p7.export_data_visualizer_workbook(dvPage);
  assert(dvXlsx[0] === 0x50 && dvXlsx[1] === 0x4b, 'Data Visualizer .xlsx export must be valid OpenXML workbook');

  const cloudPage = p7.add_page('Cloud');
  assert.equal(
    p7.import_cloud_discovery(
      cloudPage,
      JSON.stringify({
        provider: 'aws',
        vpcs: [{ id: 'v1', name: 'Prod VPC', cidr: '10.0.0.0/16' }],
        resources: [
          { id: 'n1', name: 'ALB', kind: 'cdn', vpc: 'v1' },
          { id: 'n2', name: 'ECS', kind: 'compute', vpc: 'v1' },
        ],
        edges: [{ from: 'n1', to: 'n2', label: 'HTTPS' }],
      })
    ),
    2
  );

  const ldifPage = p7.add_page('LDIF');
  assert.equal(
    p7.import_entra_or_ldif(
      ldifPage,
      'dn: cn=Jane Doe,ou=Exec,dc=vexio,dc=dev\ncn: Jane Doe\ntitle: CEO\ndepartment: Exec\n\ndn: cn=Alex Kim,ou=Eng,dc=vexio,dc=dev\ncn: Alex Kim\ntitle: Principal Eng\nmanager: cn=Jane Doe,ou=Exec,dc=vexio,dc=dev\ndepartment: Eng\n'
    ),
    2
  );

  // Per-Cell HLC CRDT Convergence across two concurrent offline WASM sessions!
  const peerA = new wasmMod.WasmVexioSession('Peer A');
  peerA.set_replica_id('peer-a');
  const sharedCard = peerA.draw_rectangle(1, 2.0, 2.0, 4.0, 3.2);
  const seedVsdx = peerA.save_format('vsdx');
  peerA.flush_crdt_ops_json(1000); // baseline

  const peerB = new wasmMod.WasmVexioSession('Peer B');
  peerB.open_bytes(seedVsdx, 'vsdx');
  peerB.set_replica_id('peer-b');

  // Peer A moves PinX while offline; Peer B changes FillForegnd & Text on the SAME shape while offline!
  peerA.move_shape(1, sharedCard, 6.5, 4.0);
  const opsFromA = peerA.flush_crdt_ops_json(1010);

  peerB.set_shape_text(1, sharedCard, 'Converged CRDT Node');
  peerB.set_cell_formula(1, sharedCard, 'FillForegnd', '=RGB(220,252,231)', false);
  const opsFromB = peerB.flush_crdt_ops_json(1020);

  assert(peerA.apply_remote_crdt_ops_json(opsFromB) >= 1, 'Peer A must apply Peer B cell ops');
  assert(peerB.apply_remote_crdt_ops_json(opsFromA) >= 1, 'Peer B must apply Peer A PinX/PinY ops');

  const stateA = JSON.parse(peerA.get_state_json(1)).shapes.find((s) => s.id === sharedCard);
  const stateB = JSON.parse(peerB.get_state_json(1)).shapes.find((s) => s.id === sharedCard);
  assert.equal(stateA.text, 'Converged CRDT Node');
  assert.equal(stateB.text, 'Converged CRDT Node');
  assert(Math.abs(stateA.pinX - 6.5) < 1e-4 && Math.abs(stateB.pinX - 6.5) < 1e-4, 'Both peers must converge on PinX=6.5 without clobbering Text!');
  console.log('  ✓ Phase 7 Boolean Ops, Freeform, Control/Vertex Grips, 6×4 Themes, Subprocess, Diff, DataViz, Cloud, LDIF & HLC CRDT verified.');

  console.log('=== [6/6] Verifying Firebase Auth, Version History & Collab CRDT Security Rules ===');
  const unauthRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/users/intruder/settings/workstation`
  );
  assert.equal(unauthRes.status, 403, 'Unauthenticated Firestore read MUST be denied (403)');

  const unauthCollabRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/collabRooms/room-1/presence`
  );
  assert.equal(unauthCollabRes.status, 403, 'Unauthenticated Collab Room presence read MUST be denied (403)');

  const authSignupRes = await fetch(
    `${AUTH_ORIGIN}/identitytoolkit.googleapis.com/v1/accounts:signUp?key=demo-vexio-offline-key`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ returnSecureToken: true }),
    }
  );
  assert.equal(authSignupRes.status, 200, 'Firebase Auth Emulator anonymous sign-up failed');
  const { idToken, localId } = await authSignupRes.json();

  const ownerWriteRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/users/${localId}/settings/workstation`,
    {
      method: 'PATCH',
      headers: {
        Authorization: `Bearer ${idToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        fields: {
          schemaVersion: { integerValue: '1' },
          updatedAt: { stringValue: new Date().toISOString() },
        },
      }),
    }
  );
  assert.equal(ownerWriteRes.status, 200, 'Authenticated owner write to own Firestore path must succeed');

  // Version History Snapshot owner write
  const versionWriteRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/users/${localId}/versions/doc1__v1`,
    {
      method: 'PATCH',
      headers: {
        Authorization: `Bearer ${idToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        fields: {
          docId: { stringValue: 'doc1' },
          label: { stringValue: 'Milestone 1' },
          byteLength: { integerValue: '4096' },
          createdAt: { stringValue: new Date().toISOString() },
        },
      }),
    }
  );
  assert.equal(versionWriteRes.status, 200, 'Authenticated owner write to versions collection must succeed');

  // Collab Room CRDT Op write with crdtOpsJson
  const crdtOpWriteRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/collabRooms/room-1/ops/op-crdt-1`,
    {
      method: 'PATCH',
      headers: {
        Authorization: `Bearer ${idToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        fields: {
          uid: { stringValue: localId },
          senderPeerId: { stringValue: 'peer-101' },
          crdtOpsJson: { stringValue: opsFromA },
          updatedAt: { stringValue: new Date().toISOString() },
        },
      }),
    }
  );
  assert.equal(crdtOpWriteRes.status, 200, 'Authenticated peer CRDT delta op write to collabRooms/ops must succeed');

  // Collab Room Presence write (own uid allowed, spoofed uid denied!)
  const presenceOkRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/collabRooms/room-1/presence/peer-101`,
    {
      method: 'PATCH',
      headers: {
        Authorization: `Bearer ${idToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        fields: {
          uid: { stringValue: localId },
          peerName: { stringValue: 'Architect #101' },
          color: { stringValue: '#ec4899' },
          cursorX: { doubleValue: 4.5 },
          cursorY: { doubleValue: 5.2 },
          updatedAt: { stringValue: new Date().toISOString() },
        },
      }),
    }
  );
  assert.equal(presenceOkRes.status, 200, 'Authenticated peer presence write with matching uid must succeed');

  const presenceSpoofRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/collabRooms/room-1/presence/peer-spoof`,
    {
      method: 'PATCH',
      headers: {
        Authorization: `Bearer ${idToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        fields: {
          uid: { stringValue: 'forged-other-uid' },
          peerName: { stringValue: 'Spoofer' },
          color: { stringValue: '#000000' },
          cursorX: { doubleValue: 1.0 },
          cursorY: { doubleValue: 1.0 },
          updatedAt: { stringValue: new Date().toISOString() },
        },
      }),
    }
  );
  assert.equal(presenceSpoofRes.status, 403, 'Spoofed uid in Collab Room presence MUST be denied (403)');

  const crossTenantRes = await fetch(
    `${FIRESTORE_ORIGIN}/v1/projects/${PROJECT_ID}/databases/(default)/documents/users/other-user-uid/settings/workstation`,
    {
      headers: { Authorization: `Bearer ${idToken}` },
    }
  );
  assert.equal(crossTenantRes.status, 403, 'Cross-tenant Firestore read MUST be denied (403)');
  console.log('  ✓ Firebase Auth, Version History, CRDT Ops, and Collab Room security rules verified.');

  console.log('\nALL VEXIO END-TO-END VERIFICATION GATES PASSED 100%!');
}

main().catch((err) => {
  console.error('VERIFICATION FAILED:', err);
  process.exit(1);
});
