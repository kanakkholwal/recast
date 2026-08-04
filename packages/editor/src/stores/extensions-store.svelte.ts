/**
 * Reactive state for installed asset-pack extensions. Mirrors the shape of
 * `assets-store.svelte.ts`: a list the Extensions panel renders, plus a busy
 * flag and last-error string for the install/uninstall UX.
 *
 * This store is the UI source of truth for *which packs exist and their
 * enabled state*. The visual entries those packs contribute live in the asset
 * registry (`lib/registry`); `lib/extensions.ts` keeps the two in sync.
 */

import type { InstalledExtension } from "$lib/ipc-types";

function createExtensionsStore() {
	let installed = $state<InstalledExtension[]>([]);
	let busy = $state(false);
	let lastError = $state<string | null>(null);

	return {
		get installed() {
			return installed;
		},
		get busy() {
			return busy;
		},
		get lastError() {
			return lastError;
		},
		setBusy(v: boolean) {
			busy = v;
		},
		setError(msg: string | null) {
			lastError = msg;
		},
		/** Replace the whole list (after a fresh enumeration). */
		setAll(list: InstalledExtension[]) {
			installed = list;
		},
		/** Insert or replace one pack by id (after install / toggle). */
		upsert(ext: InstalledExtension) {
			const idx = installed.findIndex((e) => e.manifest.id === ext.manifest.id);
			installed =
				idx >= 0
					? [...installed.slice(0, idx), ext, ...installed.slice(idx + 1)]
					: [...installed, ext];
		},
		/** Drop one pack by id (after uninstall). */
		remove(extId: string) {
			installed = installed.filter((e) => e.manifest.id !== extId);
		},
	};
}

export const extensionsStore = createExtensionsStore();
export type ExtensionsStore = ReturnType<typeof createExtensionsStore>;
