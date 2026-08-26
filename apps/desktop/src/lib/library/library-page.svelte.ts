import { deleteFile, renameFile, type RecordingEntry } from "$lib/ipc";
import type { LibraryView } from "$lib/library/card-styles";
import { filterEntries, sortEntries, sumBytes, type LibrarySort } from "$lib/library/list";
import { createSelection } from "$lib/library/selection.svelte";
import { libraryStatus } from "$lib/library/status";
import {
	createThumbnailLoader,
	removeThumbnail,
	removeThumbnails,
	renameThumbnail,
	type ThumbnailMap,
} from "$lib/library/thumbnails";
import { safeStorage } from "@recast/ui/persisted-state";
import { toast } from "@recast/ui/sonner";

interface LibraryPageConfig {
	/** Singular noun for toast copy and select mode, e.g. `recording` / `export`. */
	noun: string;
	/** `safeStorage` key for the grid/list preference. */
	viewKey: string;
	load: () => Promise<RecordingEntry[]>;
	/** Search the extension too (exports search by type; recordings don't). */
	matchExtension?: boolean;
	/** Extra cleanup when an entry leaves the list (e.g. forget cloud links). */
	onEntryRemoved?: (entry: RecordingEntry) => void;
}

/**
 * Shared state + operations for the recordings/exports listings: load, search,
 * sort, view, thumbnails, multi-select, rename and trash. Screen-specific menus
 * (migration, cloud) stay in each page and mutate through `entries`/`refresh`.
 */
export function createLibraryPage(config: LibraryPageConfig) {
	let entries = $state<RecordingEntry[]>([]);
	let isLoading = $state(true);
	let loadError = $state<string | null>(null);
	let thumbnails = $state<ThumbnailMap>({});
	let query = $state("");
	let view = $state<LibraryView>("grid");
	let sort = $state<LibrarySort>("recent");
	const loadThumbnails = createThumbnailLoader();

	function drop(paths: Set<string>) {
		if (config.onEntryRemoved) {
			for (const e of entries) if (paths.has(e.path)) config.onEntryRemoved(e);
		}
		entries = entries.filter((e) => !paths.has(e.path));
		if (paths.size > 0) thumbnails = removeThumbnails(thumbnails, paths);
	}

	const selection = createSelection({
		noun: config.noun,
		deleteFile,
		onDeleted: drop,
	});

	async function refreshThumbnails(items: RecordingEntry[]) {
		const next = await loadThumbnails(items);
		if (next) thumbnails = next;
	}

	async function refresh() {
		isLoading = true;
		try {
			entries = await config.load();
			loadError = null;
			void refreshThumbnails(entries);
		} catch (e) {
			loadError = String(e);
			toast.error(`Could not load ${config.noun}s: ${e}`);
		} finally {
			isLoading = false;
		}
	}

	/** Read the persisted view preference. Call from `onMount` (localStorage-safe). */
	function restoreView() {
		view = safeStorage.get<LibraryView>(config.viewKey, view);
	}

	async function handleRename(entry: RecordingEntry, nextName: string) {
		const newPath = await renameFile(entry.path, nextName);
		entries = entries.map((e) =>
			e.path === entry.path
				? { ...e, path: newPath, filename: newPath.split(/[\\/]/).pop() ?? nextName }
				: e,
		);
		thumbnails = renameThumbnail(thumbnails, entry.path, newPath);
		toast.success("Renamed");
	}

	async function handleDelete(entry: RecordingEntry) {
		await deleteFile(entry.path);
		config.onEntryRemoved?.(entry);
		entries = entries.filter((e) => e.path !== entry.path);
		thumbnails = removeThumbnail(thumbnails, entry.path);
		toast.success(`Moved "${entry.filename}" to trash`);
	}

	async function copyPath(entry: RecordingEntry) {
		try {
			await navigator.clipboard.writeText(entry.path);
			toast.success("Path copied");
		} catch (e) {
			toast.error(`Copy failed: ${e}`);
		}
	}

	const filtered = $derived(
		sortEntries(filterEntries(entries, query, { matchExtension: config.matchExtension }), sort),
	);
	const status = $derived(
		libraryStatus({
			loading: isLoading,
			error: loadError,
			total: entries.length,
			matches: filtered.length,
			query,
		}),
	);
	// Touch `view` so the keyed {#each} re-runs on a layout toggle and morph fires.
	const displayed = $derived.by(() => {
		void view;
		return filtered.slice();
	});

	return {
		get entries() {
			return entries;
		},
		set entries(value: RecordingEntry[]) {
			entries = value;
		},
		get isLoading() {
			return isLoading;
		},
		get loadError() {
			return loadError;
		},
		get thumbnails() {
			return thumbnails;
		},
		get query() {
			return query;
		},
		set query(value: string) {
			query = value;
		},
		get view() {
			return view;
		},
		set view(value: LibraryView) {
			view = value;
			safeStorage.set(config.viewKey, value);
		},
		get sort() {
			return sort;
		},
		set sort(value: LibrarySort) {
			sort = value;
		},
		get filtered() {
			return filtered;
		},
		get displayed() {
			return displayed;
		},
		get status() {
			return status;
		},
		get totalSize() {
			return sumBytes(entries);
		},
		get selectedCount() {
			return selection.count;
		},
		get allFilteredSelected() {
			return selection.allSelected(filtered);
		},
		selection,
		refresh,
		restoreView,
		handleRename,
		handleDelete,
		copyPath,
	};
}
