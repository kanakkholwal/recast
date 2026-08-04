/**
 * Asset-pack extension orchestration.
 *
 * Keeps three things in sync:
 *   1. the Rust installer (download/verify/persist under app_data/extensions),
 *   2. `extensionsStore` (the panel's list + busy/error state),
 *   3. the asset `registry` (the visual entries packs contribute to pickers).
 *
 * Startup: `initExtensions()` enumerates installed packs (no network) and
 * registers the enabled ones. Install/uninstall/toggle keep all three in sync.
 */

import {
	fetchExtensionRegistry,
	installExtension,
	listInstalledExtensions,
	setExtensionEnabled,
	uninstallExtension,
	type ExtensionManifest,
	type InstalledExtension,
} from "$lib/ipc";
import { log } from "$lib/logger";
import { registerExtension, unregisterExtension } from "$lib/registry/extensions";
import { isTauriApp } from "$lib/runtime/tauri";
import { extensionsStore } from "$lib/stores/extensions-store.svelte";

/** One entry of the curated registry index served from the extensions release. */
export interface RegistryIndexEntry {
	id: string;
	name: string;
	version?: string;
	author?: string;
	description?: string;
	manifestUrl: string;
	iconUrl?: string;
}

/** Compare two `x.y.z` versions → -1 / 0 / 1. Numeric 3-part compare is enough:
 *  pack versions are strict semver with no pre-release suffixes. */
export function compareSemver(a: string, b: string): number {
	const pa = a.split(".").map((n) => Number.parseInt(n, 10) || 0);
	const pb = b.split(".").map((n) => Number.parseInt(n, 10) || 0);
	for (let i = 0; i < 3; i++) {
		const d = (pa[i] ?? 0) - (pb[i] ?? 0);
		if (d !== 0) return d < 0 ? -1 : 1;
	}
	return 0;
}

/** True when `latest` is a strictly newer version than the `installed` one. */
export function hasUpdate(installed: string, latest: string | undefined): boolean {
	return !!latest && compareSemver(latest, installed) > 0;
}

/** Curated registry index URL (browse gallery). Override via env. */
const DEFAULT_REGISTRY_INDEX_URL =
	"https://github.com/kanakkholwal/recast/releases/download/extensions-v1/index.json";

export function registryIndexUrl(): string {
	const fromEnv = import.meta.env?.PUBLIC_EXTENSIONS_INDEX_URL;
	return typeof fromEnv === "string" && fromEnv.length > 0
		? fromEnv
		: DEFAULT_REGISTRY_INDEX_URL;
}

let initialised = false;

/** Enumerate installed packs and register the enabled ones. No network. */
async function hydrate(): Promise<void> {
	if (!(await isTauriApp())) return;
	try {
		const list = await listInstalledExtensions();
		extensionsStore.setAll(list);
		await Promise.all(list.map((ext) => registerExtension(ext)));
	} catch (err) {
		log.warn("extensions", "hydrate_failed", { err: String(err) });
	}
}

/** Call once from the root layout. Idempotent. */
export function initExtensions(): void {
	if (initialised) return;
	initialised = true;
	void hydrate();
}

/** Install (or update) a pack from a manifest URL, then register it. */
export async function installFromUrl(manifestUrl: string): Promise<InstalledExtension> {
	extensionsStore.setBusy(true);
	extensionsStore.setError(null);
	try {
		const ext = await installExtension(manifestUrl.trim());
		extensionsStore.upsert(ext);
		await registerExtension(ext);
		return ext;
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		extensionsStore.setError(msg);
		throw err;
	} finally {
		extensionsStore.setBusy(false);
	}
}

/** Remove a pack and drop its registry entries. */
export async function removeExtension(extId: string): Promise<void> {
	extensionsStore.setBusy(true);
	try {
		await uninstallExtension(extId);
		unregisterExtension(extId);
		extensionsStore.remove(extId);
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		extensionsStore.setError(msg);
		throw err;
	} finally {
		extensionsStore.setBusy(false);
	}
}

/** Enable/disable a pack, updating both the store and the registry. */
export async function toggleExtension(extId: string, enabled: boolean): Promise<void> {
	extensionsStore.setBusy(true);
	try {
		await setExtensionEnabled(extId, enabled);
		const current = extensionsStore.installed.find((e) => e.manifest.id === extId);
		if (current) {
			const next = { ...current, enabled };
			extensionsStore.upsert(next);
			await registerExtension(next); // registers when enabled, clears when not
		}
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		extensionsStore.setError(msg);
		throw err;
	} finally {
		extensionsStore.setBusy(false);
	}
}

/** Fetch the curated registry index for the browse gallery. */
export async function loadRegistryIndex<T = unknown>(): Promise<T | null> {
	if (!(await isTauriApp())) return null;
	try {
		return await fetchExtensionRegistry<T>(registryIndexUrl());
	} catch (err) {
		log.warn("extensions", "registry_index_failed", { err: String(err) });
		return null;
	}
}

/** Fetch a pack's full manifest for the pre-install details preview. Reuses the
 *  URL-allowlisted registry fetch, so the same https/localhost gate applies. */
export async function fetchManifestPreview(
	manifestUrl: string,
): Promise<ExtensionManifest | null> {
	if (!(await isTauriApp())) return null;
	try {
		return await fetchExtensionRegistry<ExtensionManifest>(manifestUrl.trim());
	} catch (err) {
		log.warn("extensions", "manifest_preview_failed", { err: String(err) });
		return null;
	}
}
