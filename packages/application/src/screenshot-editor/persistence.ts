import type { CustomPreset, DesignObject, EditorSnapshot } from "./types";

/** Autosave draft in IndexedDB (images are data-URLs, too large for
 * localStorage). Custom presets are design-only, so they live in localStorage. */

const DB_NAME = "recast-screenshot-editor";
const STORE = "drafts";
const DRAFT_KEY = "current";
const PRESETS_KEY = "recast-screenshot-presets";

function hasIdb(): boolean {
	return typeof indexedDB !== "undefined";
}

function openDb(): Promise<IDBDatabase> {
	return new Promise((resolve, reject) => {
		const req = indexedDB.open(DB_NAME, 1);
		req.onupgradeneeded = () => {
			if (!req.result.objectStoreNames.contains(STORE)) req.result.createObjectStore(STORE);
		};
		req.onsuccess = () => resolve(req.result);
		req.onerror = () => reject(req.error);
	});
}

function tx<T>(
	mode: IDBTransactionMode,
	run: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
	return openDb().then(
		(db) =>
			new Promise<T>((resolve, reject) => {
				const t = db.transaction(STORE, mode);
				const req = run(t.objectStore(STORE));
				req.onsuccess = () => resolve(req.result);
				req.onerror = () => reject(req.error);
				t.oncomplete = () => db.close();
			}),
	);
}

export async function saveDraft(snapshot: EditorSnapshot): Promise<void> {
	if (!hasIdb()) return;
	await tx("readwrite", (s) => s.put(snapshot, DRAFT_KEY));
}

export async function loadDraft(): Promise<EditorSnapshot | null> {
	if (!hasIdb()) return null;
	try {
		const v = await tx<EditorSnapshot | undefined>("readonly", (s) => s.get(DRAFT_KEY));
		return v ?? null;
	} catch {
		return null;
	}
}

export async function clearDraft(): Promise<void> {
	if (!hasIdb()) return;
	try {
		await tx("readwrite", (s) => s.delete(DRAFT_KEY));
	} catch {
		// best-effort
	}
}

// --- Custom presets (localStorage) ---------------------------------------

export function listCustomPresets(): CustomPreset[] {
	if (typeof localStorage === "undefined") return [];
	try {
		const raw = localStorage.getItem(PRESETS_KEY);
		return raw ? (JSON.parse(raw) as CustomPreset[]) : [];
	} catch {
		return [];
	}
}

function writePresets(list: CustomPreset[]): void {
	if (typeof localStorage === "undefined") return;
	localStorage.setItem(PRESETS_KEY, JSON.stringify(list));
}

export function saveCustomPreset(name: string, design: DesignObject, now: number): CustomPreset {
	const preset: CustomPreset = {
		id: `preset-${now}-${Math.round(now % 100000)}`,
		name: name.trim() || "Untitled",
		createdAt: now,
		design,
	};
	writePresets([preset, ...listCustomPresets()]);
	return preset;
}

export function deleteCustomPreset(id: string): void {
	writePresets(listCustomPresets().filter((p) => p.id !== id));
}
