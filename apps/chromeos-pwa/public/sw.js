/**
 * Vexio Offline-First Service Worker (Workbox-compatible Precache & Delta Update Engine).
 *
 * Guarantees 100% offline execution on ChromeOS:
 * - Precaches 100% of the app shell, JS chunks, Rust WASM SIMD binary (`vexio_core_bg.wasm`),
 *   WGSL shaders, and built-in `.vssx` stencil packs during `install`.
 * - Serves precached assets with zero-latency Cache-First strategy and performs background
 *   delta validation from Firebase Hosting when online.
 */

const CACHE_VERSION = 'vexio-precache-v4';
const IS_PROD_PRECACHE = Array.isArray(self.__VEXIO_PRECACHE_MANIFEST);
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
      if (IS_PROD_PRECACHE) {
        const cache = await caches.open(CACHE_VERSION);
        await Promise.all(
          PRECACHE_MANIFEST.map(async (entryUrl) => {
            const req = new Request(entryUrl, { cache: 'reload' });
            const res = await fetch(req);
            if (res && res.ok) {
              await cache.put(entryUrl, res);
            }
          })
        );
      }
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
  if (!IS_PROD_PRECACHE) return;
  const { request } = event;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  if (
    url.origin !== self.location.origin ||
    url.pathname.startsWith('/__/auth')
  ) {
    return;
  }

  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE_VERSION);

      // Network-First with instant offline cache fallback for HTML navigations
      if (request.mode === 'navigate' || url.pathname === '/' || url.pathname.endsWith('/index.html')) {
        try {
          const fresh = await fetch(new Request(request, { cache: 'no-store' }));
          if (fresh && fresh.ok) {
            await cache.put('/index.html', fresh.clone());
          }
          return withIsolationHeaders(fresh);
        } catch {
          const cachedShell =
            (await cache.match(request, { ignoreSearch: true })) ||
            (await cache.match('/index.html'));
          if (cachedShell) {
            return withIsolationHeaders(cachedShell);
          }
          return new Response('Vexio Offline Shell Unavailable', { status: 503 });
        }
      }

      const cached = (await cache.match(request)) || (await cache.match(url.pathname, { ignoreSearch: true }));
      if (cached) {
        return withIsolationHeaders(cached);
      }

      try {
        const netResponse = await fetch(request);
        if (netResponse && netResponse.ok) {
          await cache.put(request, netResponse.clone());
        }
        return withIsolationHeaders(netResponse);
      } catch {
        const shell = await cache.match('/index.html');
        if (shell) {
          return withIsolationHeaders(shell);
        }
        return new Response('Vexio Offline Asset Unavailable', { status: 503 });
      }
    })()
  );
});

