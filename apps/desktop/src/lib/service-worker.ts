// Swap DOM typings for worker globals.
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />

import { build, files, version } from '$service-worker';

const self = globalThis.self as unknown as ServiceWorkerGlobalScope;
const isTauri =
    (self.location.protocol.includes('tauri') ||
    self.location.hostname.includes('tauri.localhost'));
const CACHE = `recast.nexonauts.cache-${version}`;

const ASSETS = [
    ...build, // the app itself
    ...files  // everything in `static`
];

self.addEventListener('install', (event) => {
    async function addFilesToCache() {
        const cache = await caches.open(CACHE);
        return await cache.addAll(ASSETS);
    }

    event.waitUntil(addFilesToCache());
});

self.addEventListener('activate', (event) => {
    async function deleteOldCaches() {
        for (const key of await caches.keys()) {
            if (key !== CACHE) await caches.delete(key);
        }
    }

    event.waitUntil(deleteOldCaches());
});

self.addEventListener('fetch', (event) => {
    if (event.request.method !== 'GET') return;

    // Only handle http/https; ignore chrome-extension://, blob://, data://, etc.
    const { protocol } = new URL(event.request.url);
    if (protocol !== 'http:' && protocol !== 'https:' && !isTauri) return;


    async function respond() {
        const url = new URL(event.request.url);
        const cache = await caches.open(CACHE);

        // Internal assets (build/files) always come from cache.
        if (ASSETS.includes(url.pathname)) {
            const response = await cache.match(url.pathname);
            if (response) {
                return response;
            }
        }

        // Heavy external assets: cache-first.
        const isHeavyExternalAsset =
            url.hostname === 'unpkg.com' ||
            url.hostname === 'cdn.jsdelivr.net' ||
            url.pathname.endsWith('.wasm') ||
            url.pathname.endsWith('.worker.js');

        if (isHeavyExternalAsset) {
            const cachedResponse = await cache.match(event.request);
            if (cachedResponse) {
                return cachedResponse;
            }

            try {
                const networkResponse = await fetch(event.request);
                if (networkResponse.status === 200) {
                    cache.put(event.request, networkResponse.clone());
                }
                return networkResponse;
            } catch (err) {
                throw err;
            }
        }

        // Everything else: network-first, fall back to cache.
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
