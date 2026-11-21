import { cpSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const distDir = fileURLToPath(new URL('../dist', import.meta.url));
const firebaseDistDir = fileURLToPath(new URL('../../../firebase/dist', import.meta.url));

function walk(dir) {
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      results.push(...walk(full));
    } else if (entry !== 'sw.js') {
      const rel = '/' + relative(distDir, full).replace(/\\/g, '/');
      results.push(rel);
    }
  }
  return results.sort();
}

const files = ['/', ...walk(distDir)];
const hash = createHash('sha256');
for (const f of files) {
  if (f === '/') continue;
  hash.update(f);
  hash.update(readFileSync(join(distDir, f.slice(1))));
}
const buildRevision = hash.digest('hex').slice(0, 12);

const swPath = join(distDir, 'sw.js');
let swSource = readFileSync(swPath, 'utf8');
swSource = swSource
  .replace(/vexio-precache-v\d+/, `vexio-precache-${buildRevision}`)
  .replace(
    'self.__VEXIO_PRECACHE_MANIFEST',
    JSON.stringify(files, null, 2)
  );
writeFileSync(swPath, swSource, 'utf8');
rmSync(firebaseDistDir, { recursive: true, force: true });
cpSync(distDir, firebaseDistDir, { recursive: true });
console.log(
  `[vexio-precache] Injected ${files.length} offline precache entries (revision: ${buildRevision}) into dist/sw.js and mirrored to firebase/dist`
);
