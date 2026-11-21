import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const wasmPath = fileURLToPath(new URL('./public/wasm/vexio_core_bg.wasm', import.meta.url));
const wasmBuildHash = existsSync(wasmPath)
  ? createHash('sha256').update(readFileSync(wasmPath)).digest('hex').slice(0, 12)
  : Date.now().toString(36);

export default defineConfig({
  define: {
    __VEXIO_WASM_HASH__: JSON.stringify(wasmBuildHash),
  },
  server: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Resource-Policy': 'same-origin',
    },
  },
  preview: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Resource-Policy': 'same-origin',
    },
  },
  build: {
    target: 'es2022',
    outDir: 'dist',
    assetsInlineLimit: 0,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/@firebase/firestore') || id.includes('node_modules/firebase/firestore')) {
            return 'firebase-firestore';
          }
          if (id.includes('node_modules/@firebase/auth') || id.includes('node_modules/firebase/auth')) {
            return 'firebase-auth';
          }
          if (id.includes('node_modules/@firebase/storage') || id.includes('node_modules/firebase/storage')) {
            return 'firebase-storage';
          }
          if (id.includes('node_modules/@firebase') || id.includes('node_modules/firebase')) {
            return 'firebase-core';
          }
          return undefined;
        },
      },
    },
  },
  worker: {
    format: 'es',
  },
});
