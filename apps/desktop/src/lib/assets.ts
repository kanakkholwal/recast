/**
 * External-asset download/cache helper.
 *
 * Flow on startup:
 *   1. `hydrateCachedAssets()` — synchronous (no network) read of the on-disk
 *      lock file. Populates the store with whatever is already cached so the
 *      UI upgrades past the CSS placeholder the moment it paints, even when
 *      the user is offline.
 *   2. `ensureAssetsInstalled(manifestUrl)` — network fetch of the manifest,
 *      then SHA-256-verified download of missing/mismatched assets. Thumbs
 *      download first (tiny) so the picker grid becomes usable fast.
 *   3. On `window.online` — retry any failed downloads automatically.
 *
 * Manifest URL comes from `PUBLIC_ASSETS_MANIFEST_URL` (Vite env). Falls back
 * to the main `wallpapers-v1` release so dev builds work out of the box.
 */

import { isTauriApp } from "$lib/runtime/tauri";
import {
	ensureAssetsInstalled,
	getCachedAssetPath,
	hydrateCachedAssets,
} from "$lib/ipc";
import { assetsStore } from "$lib/stores/assets-store.svelte";

const DEFAULT_MANIFEST_URL =
	"https://github.com/kanakkholwal/recast/releases/download/wallpapers-v1/manifest.json";

function manifestUrl(): string {
	const fromEnv = import.meta.env?.PUBLIC_ASSETS_MANIFEST_URL;
	return typeof fromEnv === "string" && fromEnv.length > 0 ? fromEnv : DEFAULT_MANIFEST_URL;
}

let initialised = false;
let inFlight: Promise<void> | null = null;
let hydrated = false;

/** Populate the store from the persisted lock file without touching the network. */
async function hydrateFromDisk(): Promise<void> {
	if (hydrated) return;
	if (!(await isTauriApp())) {
		hydrated = true;
		return;
	}
	try {
		const entries = await hydrateCachedAssets();
		for (const entry of entries) {
			if (entry.path) assetsStore.setPath(entry.id, entry.path);
			if (entry.thumbPath) assetsStore.setThumbPath(entry.id, entry.thumbPath);
		}
	} catch (err) {
		console.warn("asset hydrate failed:", err);
	}
	hydrated = true;
}

async function runInstall(): Promise<void> {
	if (!(await isTauriApp())) return;
	await hydrateFromDisk();
	assetsStore.setInstalling(true);
	assetsStore.setError(null);
	try {
		const result = await ensureAssetsInstalled(manifestUrl());
		for (const entry of result.hydrated) {
			if (entry.path) assetsStore.setPath(entry.id, entry.path);
			if (entry.thumbPath) assetsStore.setThumbPath(entry.id, entry.thumbPath);
		}
		assetsStore.setFailed(result.failed.map((f) => f.id));
		if (result.failed.length > 0) {
			const first = result.failed[0];
			assetsStore.setError(`${first.id}: ${first.reason}`);
		}
		assetsStore.setReady(true);
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		assetsStore.setError(msg);
	} finally {
		assetsStore.setInstalling(false);
	}
}

/** Kick off an install. Deduped so repeat calls share the in-flight promise. */
export function ensureAssets(): Promise<void> {
	if (inFlight) return inFlight;
	inFlight = runInstall().finally(() => {
		inFlight = null;
	});
	return inFlight;
}

/** Returns the cached absolute path for an asset id, or null if not yet cached. */
export async function resolveAsset(id: string): Promise<string | null> {
	const cached = assetsStore.paths[id];
	if (cached) return cached;
	if (!(await isTauriApp())) return null;
	const path = await getCachedAssetPath(id);
	if (path) assetsStore.setPath(id, path);
	return path;
}

/** Retry downloads whenever the browser reports connectivity restored. */
export function retryOnReconnect() {
	if (assetsStore.failed.length === 0 && assetsStore.ready) return;
	void ensureAssets();
}

/** Call once from the root layout. Idempotent. */
export function initAssets() {
	if (initialised) return;
	initialised = true;
	void hydrateFromDisk().then(() => ensureAssets());
	if (typeof window !== "undefined") {
		window.addEventListener("online", retryOnReconnect);
	}
}
