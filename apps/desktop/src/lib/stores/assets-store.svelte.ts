/**
 * Reactive state for the external-asset cache. Holds per-id cached file paths
 * plus install progress. Consumed by LazyExternalImage and the render pipeline
 * to upgrade placeholder thumbs to full-res downloaded assets.
 */

function createAssetsStore() {
	let paths = $state<Record<string, string>>({});
	let ready = $state(false);
	let installing = $state(false);
	let failed = $state<string[]>([]);
	let lastError = $state<string | null>(null);

	return {
		get paths() {
			return paths;
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
		clearPath(id: string) {
			if (id in paths) {
				const next = { ...paths };
				delete next[id];
				paths = next;
			}
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
