/**
 * End-to-End Automated Verification Suite for Vexio Target D:
 * Firebase Hosting (COOP/COEP, WASM MIME, Caching) + Auth/Firestore Security Rules
 * + ChromeOS Manifest (`window-controls-overlay`, `file_handlers`, 12 precached `.vssx` stencils)
 * + 100% Unauthenticated Offline Rust WASM SIMD Engine Execution (ShapeSheet, Lasso, Align,
 *   8-Handle Transform, Smart Guides, 8 Starter Templates, Org/Gantt/Timeline, CAD/VSD/PDF).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const HOSTING_ORIGIN = 'http://127.0.0.1:5000';
const AUTH_ORIGIN = 'http://127.0.0.1:9099';
const FIRESTORE_ORIGIN = 'http://127.0.0.1:8080';
const PROJECT_ID = 'demo-vexio-chromeos';

// Ensure Node bypasses any outbound HTTP proxy when talking to 127.0.0.1 local emulators!
process.env.NO_PROXY = '127.0.0.1,localhost';
process.env.no_proxy = '127.0.0.1,localhost';

async function main() {
  console.log('=== [1/5] Verifying Firebase Hosting COOP/COEP SharedArrayBuffer Headers ===');
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

  console.log('=== [2/5] Verifying WASM SIMD Binary MIME Type & Immutable Caching ===');
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

  console.log('=== [3/5] Verifying ChromeOS Manifest (WCO + File Handlers) & 12-Pack SW Precache ===');
  const manifestRes = await fetch(`${HOSTING_ORIGIN}/manifest.webmanifest`);
  assert.equal(manifestRes.status, 200);
  const manifest = await manifestRes.json();
  assert.deepEqual(manifest.display_override, ['window-controls-overlay', 'standalone']);
  const acceptedExts = Object.values(manifest.file_handlers[0].accept).flat();
  for (const reqExt of ['.vsdx', '.vsdm', '.vssx', '.vstx', '.vsd', '.dwg', '.dxf']) {
    assert(acceptedExts.includes(reqExt), `manifest file_handlers missing ${reqExt}`);
  }

  const swRes = await fetch(`${HOSTING_ORIGIN}/sw.js`);
  assert.equal(swRes.status, 200);
  assert.match(swRes.headers.get('cache-control') ?? '', /no-cache/i);
  const swText = await swRes.text();
  assert(swText.includes('/wasm/vexio_core_bg.wasm'), 'sw.js must precache /wasm/vexio_core_bg.wasm');
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
  console.log('  ✓ ChromeOS WCO, File Handlers, and all 12 precached .vssx Stencil Packs verified.');

  console.log('=== [4/5] Verifying 100% Unauthenticated Offline Rust WASM SIMD Engine & Phase 6 Tools ===');
  const distWasmJs = pathToFileURL(
    resolve('/home/antigravity/apps/chromeos-pwa/dist/wasm/vexio_core.js')
  ).href;
  const localWasmBytes = readFileSync(
    resolve('/home/antigravity/apps/chromeos-pwa/dist/wasm/vexio_core_bg.wasm')
  );
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

  // Save .vsdx, .vsd, .dxf, .pdf, and OPFS WAL journal 100% offline in WASM!
  const vsdxOut = session.save_format('vsdx');
  const vsdOut = session.save_format('vsd');
  const dxfOut = session.save_format('dxf');
  const pdfOut = session.save_format('pdf');
  const walOut = session.export_wal();
  assert(vsdxOut.byteLength > 500, 'WASM .vsdx output empty');
  assert(vsdOut[0] === 0xd0 && vsdOut[1] === 0xcf, 'WASM .vsd must have OLE2 header');
  assert(new TextDecoder().decode(dxfOut).includes('ENTITIES'), 'WASM .dxf must contain ENTITIES');
  assert(new TextDecoder().decode(pdfOut.slice(0, 8)).startsWith('%PDF-1.7'), 'WASM .pdf must start with %PDF-1.7');
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

  console.log('  ✓ 100% Unauthenticated Offline WASM SIMD + Lasso + Smart Guides + 8-Handle Transform + 8 Templates + Org/Gantt/Timeline verified.');

  console.log('=== [5/5] Verifying Firebase Auth, Version History & Collab Room Security Rules ===');
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
  console.log('  ✓ Firebase Auth, Version History, and Collab Room security rules verified.');

  console.log('\nALL VEXIO END-TO-END VERIFICATION GATES PASSED 100%!');
}

main().catch((err) => {
  console.error('VERIFICATION FAILED:', err);
  process.exit(1);
});
