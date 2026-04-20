/**
 * Reactive state for the external-asset cache. Tracks two tiers per asset id:
 *  - `paths[id]`       — the full-resolution cached file (preferred)
 *  - `thumbPaths[id]`  — the low-res WebP thumbnail (used until full-res lands)
 *
 * Components prefer `paths`, fall back to `thumbPaths`, and as a last resort
 * render a pure-CSS placeholder.
 */

function createAssetsStore() {
	let paths = $state<Record<string, string>>({});
	let thumbPaths = $state<Record<string, string>>({});
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
			paths = { ...paths, [id]: path };
		},
		setThumbPath(id: string, path: string) {
			thumbPaths = { ...thumbPaths, [id]: path };
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
