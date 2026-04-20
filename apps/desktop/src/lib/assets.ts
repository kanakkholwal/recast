/**
 * External-asset download/cache helper. Wraps the Rust `ensure_assets_installed`
 * command, publishes results to `assetsStore`, and exposes a reconnect-retry hook
 * so missing assets self-heal when connectivity returns.
 *
 * The manifest URL is read from the `PUBLIC_ASSETS_MANIFEST_URL` Vite env var so
 * it can be changed without a code edit (different releases, forks, staging).
 */

import { isTauriApp } from "$lib/runtime/tauri";
import { ensureAssetsInstalled, getCachedAssetPath } from "$lib/ipc";
import { assetsStore } from "$lib/stores/assets-store.svelte";

const DEFAULT_MANIFEST_URL =
	"https://github.com/kanakkholwal/recast/releases/download/wallpapers-v1/manifest.json";

function manifestUrl(): string {
	const fromEnv = import.meta.env?.PUBLIC_ASSETS_MANIFEST_URL;
	return typeof fromEnv === "string" && fromEnv.length > 0 ? fromEnv : DEFAULT_MANIFEST_URL;
}

let initialised = false;
let inFlight: Promise<void> | null = null;

async function runInstall(): Promise<void> {
	if (!(await isTauriApp())) return;
	assetsStore.setInstalling(true);
	assetsStore.setError(null);
	try {
		const result = await ensureAssetsInstalled(manifestUrl());
		for (const id of [...result.installed, ...result.skipped]) {
			const path = await getCachedAssetPath(id);
			if (path) assetsStore.setPath(id, path);
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
	void ensureAssets();
	if (typeof window !== "undefined") {
		window.addEventListener("online", retryOnReconnect);
	}
}
