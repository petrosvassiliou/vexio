/**
 * Vexio Offline-First Service Worker (Workbox-compatible Precache & Delta Update Engine).
 *
 * Guarantees 100% offline execution on ChromeOS:
 * - Precaches 100% of the app shell, JS chunks, Rust WASM SIMD binary (`vexio_core_bg.wasm`),
 *   WGSL shaders, and built-in `.vssx` stencil packs during `install`.
 * - Serves precached assets with zero-latency Cache-First strategy and performs background
 *   delta validation from Firebase Hosting when online.
 */

const CACHE_VERSION = 'vexio-precache-v1';
// Replaced at build time by `scripts/inject-precache-manifest.mjs` with exact hashed dist paths + WASM + stencils.
const PRECACHE_MANIFEST = self.__VEXIO_PRECACHE_MANIFEST || [
  '/',
  '/index.html',
  '/manifest.webmanifest',
  '/icons/vexio-icon.svg',
  '/icons/vexio-maskable.svg',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE_VERSION);
      await cache.addAll(PRECACHE_MANIFEST);
      await self.skipWaiting();
    })()
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(
        keys
          .filter((key) => key !== CACHE_VERSION)
          .map((staleKey) => caches.delete(staleKey))
      );
      await self.clients.claim();
    })()
  );
});

/**
 * Ensure Cross-Origin Isolation headers (`COOP: same-origin`, `COEP: require-corp`)
 * are preserved even when serving cached offline responses so `SharedArrayBuffer`
 * multi-threaded Rust WASM works 100% offline!
 */
function withIsolationHeaders(response) {
  if (!response || response.type === 'opaque') {
    return response;
  }
  const headers = new Headers(response.headers);
  headers.set('Cross-Origin-Opener-Policy', 'same-origin');
  headers.set('Cross-Origin-Embedder-Policy', 'require-corp');
  headers.set('Cross-Origin-Resource-Policy', 'same-origin');
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  // Allow Firebase Auth / Firestore / Storage network traffic to use Firebase SDK's own IndexedDB offline queue.
  if (
    url.origin !== self.location.origin ||
    url.pathname.startsWith('/__/auth')
  ) {
    return;
  }

  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE_VERSION);
      const cached = await cache.match(request, { ignoreSearch: request.mode === 'navigate' });

      if (cached) {
        // Background Stale-While-Revalidate delta refresh for mutable shell entry points when online
        if (url.pathname === '/' || url.pathname.endsWith('index.html')) {
          event.waitUntil(
            fetch(request)
              .then((fresh) => {
                if (fresh && fresh.ok) {
                  return cache.put(request, fresh.clone());
                }
              })
              .catch(() => {})
          );
        }
        return withIsolationHeaders(cached);
      }

      try {
        const netResponse = await fetch(request);
        if (netResponse && netResponse.ok) {
          await cache.put(request, netResponse.clone());
        }
        return withIsolationHeaders(netResponse);
      } catch {
        // Guaranteed SPA offline fallback for navigation launches (e.g., `?open-file=1` from ChromeOS Files!)
        const shell = await cache.match('/index.html');
        if (shell) {
          return withIsolationHeaders(shell);
        }
        return new Response('Vexio Offline Shell Unavailable', { status: 503 });
      }
    })()
  );
});
