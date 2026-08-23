// Mint Agent (Web) service worker.
//
// Scope is deliberately narrow: this only makes the app installable and
// keeps the last-loaded UI shell available when the network drops mid-use —
// it is not an offline-first app. `/api/*` (chat, agent tool calls, everything
// that talks to the local mint-core backend) is always network-only: caching
// or replaying agent responses would be actively wrong, not just stale.
//
// CACHE_NAME only needs bumping when this file's own caching *logic*
// changes (to drop old runtime-cache entries on activate) — individual
// built assets don't need that, since Vite already content-hashes their
// filenames, so a new deploy's JS/CSS naturally misses the old cache and
// gets fetched fresh.
const CACHE_NAME = 'mint-web-runtime-v1';
const APP_SHELL_URL = '/index-web.html';

self.addEventListener('install', () => {
  // Don't precache eagerly — there is no build-time manifest of hashed
  // asset URLs available to a hand-written service worker, and precaching
  // the wrong/stale set on install would just waste bandwidth. Assets are
  // cached opportunistically as they're actually requested instead (see
  // `staleWhileRevalidate` below).
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(
        keys.filter((key) => key !== CACHE_NAME).map((key) => caches.delete(key))
      );
      await self.clients.claim();
    })()
  );
});

async function staleWhileRevalidate(request) {
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(request);
  const networkFetch = fetch(request)
    .then((response) => {
      // Only cache real, successful, same-origin responses — an opaque
      // cross-origin response (e.g. the Google Fonts CSS/woff2) can still
      // be cached and served (the browser handles opaque responses fine),
      // but a network error or a 4xx/5xx must not overwrite a good cached
      // copy.
      if (response && (response.ok || response.type === 'opaque')) {
        cache.put(request, response.clone());
      }
      return response;
    })
    .catch(() => undefined);
  return cached ?? (await networkFetch) ?? Response.error();
}

async function networkFirstShell(request) {
  const cache = await caches.open(CACHE_NAME);
  try {
    const response = await fetch(request);
    if (response.ok) {
      cache.put(APP_SHELL_URL, response.clone());
    }
    return response;
  } catch {
    const cachedShell = await cache.match(APP_SHELL_URL);
    return cachedShell ?? Response.error();
  }
}

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') {
    return; // let non-GET requests (chat sends, tool calls, ...) through untouched
  }

  const url = new URL(request.url);
  if (url.pathname.startsWith('/api/')) {
    return; // never intercept the backend — always live
  }

  if (request.mode === 'navigate') {
    event.respondWith(networkFirstShell(request));
    return;
  }

  if (url.origin === self.location.origin || url.hostname.endsWith('gstatic.com') || url.hostname.endsWith('googleapis.com')) {
    event.respondWith(staleWhileRevalidate(request));
  }
});
