/**
 * Reactive state for the external-asset cache. Tracks two tiers per asset id:
 *  - `paths[id]`       — the full-resolution cached file (preferred)
 *  - `thumbPaths[id]`  — the low-res WebP thumbnail (used until full-res lands)
 *
 * The corresponding WebView-loadable URLs (`urls`/`thumbUrls`) are cached the
 * moment a path is set so consumers don't recompute `convertFileSrc` on every
 * re-render. URL identity is stable across reads, which is what lets
 * `$derived(src)` short-circuit and stop wallpaper grids from re-creating
 * their `<img>` decode work on every tab toggle.
 */

import { convertFileSrc } from "@tauri-apps/api/core";

function createAssetsStore() {
	let paths = $state<Record<string, string>>({});
	let thumbPaths = $state<Record<string, string>>({});
	let urls = $state<Record<string, string>>({});
	let thumbUrls = $state<Record<string, string>>({});
	let ready = $state(false);
	let installing = $state(false);
	let failed = $state<string[]>([]);
	let lastError = $state<string | null>(null);

	return {
		get paths() {
			return paths;
		},
		get thumbPaths() {
			return thumbPaths;
		},
		/** Pre-converted WebView URLs (`http://asset.localhost/...`) per id. */
		get urls() {
			return urls;
		},
		get thumbUrls() {
			return thumbUrls;
		},
		get ready() {
			return ready;
		},
		get installing() {
			return installing;
		},
		get failed() {
			return failed;
		},
		get lastError() {
			return lastError;
		},
		setInstalling(v: boolean) {
			installing = v;
		},
		setReady(v: boolean) {
			ready = v;
		},
		setPath(id: string, path: string) {
			if (paths[id] === path) return;
			paths = { ...paths, [id]: path };
			urls = { ...urls, [id]: convertFileSrc(path) };
		},
		setThumbPath(id: string, path: string) {
			if (thumbPaths[id] === path) return;
			thumbPaths = { ...thumbPaths, [id]: path };
			thumbUrls = { ...thumbUrls, [id]: convertFileSrc(path) };
		},
		setFailed(ids: string[]) {
			failed = ids;
		},
		setError(msg: string | null) {
			lastError = msg;
		},
	};
}

export const assetsStore = createAssetsStore();
export type AssetsStore = ReturnType<typeof createAssetsStore>;
