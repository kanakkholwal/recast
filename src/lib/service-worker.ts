// Disables access to DOM typings like `HTMLElement` which are not available
// inside a service worker and instantiates the correct globals
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />

// Ensures that the `$service-worker` import has proper type definitions
/// <reference types="@sveltejs/kit" />

import { build, files, version } from '$service-worker';

// This gives `self` the correct types
const self = globalThis.self as unknown as ServiceWorkerGlobalScope;
const isTauri =
    (self.location.protocol.includes('tauri') ||
    self.location.hostname.includes('tauri.localhost'));
// Create a unique cache name for this deployment
const CACHE = `orbit.nexonauts.cache-${version}`;

const ASSETS = [
    ...build, // the app itself
    ...files  // everything in `static`
];

self.addEventListener('install', (event) => {
    // Create a new cache and add all files to it
    async function addFilesToCache() {
        const cache = await caches.open(CACHE);
        return await cache.addAll(ASSETS);
    }

    event.waitUntil(addFilesToCache());
});

self.addEventListener('activate', (event) => {
    // Remove previous cached data from disk
    async function deleteOldCaches() {
        for (const key of await caches.keys()) {
            if (key !== CACHE) await caches.delete(key);
        }
    }

    event.waitUntil(deleteOldCaches());
});

self.addEventListener('fetch', (event) => {
    // ignore POST requests etc
    if (event.request.method !== 'GET') return;

    // Only handle http/https — ignore chrome-extension://, blob://, data://, etc.
    const { protocol } = new URL(event.request.url);
    if (protocol !== 'http:' && protocol !== 'https:' && !isTauri) return;


    async function respond() {
        const url = new URL(event.request.url);
        const cache = await caches.open(CACHE);

        // 1. INTERNAL ASSETS: `build`/`files` can always be served from the cache
        if (ASSETS.includes(url.pathname)) {
            const response = await cache.match(url.pathname);
            if (response) {
                return response;
            }
        }

        // 2. HEAVY EXTERNAL ASSETS: Cache-First strategy
        // Add domains or file extensions you want to load instantly from cache after the first download
        const isHeavyExternalAsset =
            url.hostname === 'unpkg.com' ||
            url.hostname === 'cdn.jsdelivr.net' ||
            url.pathname.endsWith('.wasm') ||
            url.pathname.endsWith('.worker.js');

        if (isHeavyExternalAsset) {
            const cachedResponse = await cache.match(event.request);
            if (cachedResponse) {
                return cachedResponse; // Serve instantly from disk!
            }

            try {
                const networkResponse = await fetch(event.request);
                // Only cache successful, non-opaque responses
                if (networkResponse.status === 200) {
                    cache.put(event.request, networkResponse.clone());
                }
                return networkResponse;
            } catch (err) {
                throw err;
            }
        }

        // 3. EVERYTHING ELSE: Network-First fallback (API calls, HTML pages, etc.)
        try {
            const response = await fetch(event.request);

            if (!(response instanceof Response)) {
                throw new Error('invalid response from fetch',{ cause: { response } });
            }

            if (response.status === 200) {
                cache.put(event.request, response.clone());
            }

            return response;
        } catch (err) {
            const response = await cache.match(event.request);

            if (response) {
                return response;
            }

            throw err;
        }
    }

    event.respondWith(respond());
});