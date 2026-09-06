import { safeStorage } from "@recast/ui/persisted-state";
import { toast } from "@recast/ui/sonner";
import { listExports, listRecasts, type RecordingEntry } from "$lib/ipc";
import type { LibraryView } from "$lib/library/card-styles";
import { type LibrarySort, sumBytes } from "$lib/library/list";
import { libraryStatus } from "$lib/library/status";
import { createThumbnailLoader, type ThumbnailMap } from "$lib/library/thumbnails";
import {
	countByKind,
	filterMedia,
	type MediaItem,
	type MediaTab,
	sortMedia,
	toItems,
} from "./media.logic";

const VIEW_KEY = "media-view";

export function createMediaState() {
	let recordings = $state<RecordingEntry[]>([]);
	let exports = $state<RecordingEntry[]>([]);
	let isLoading = $state(true);
	let loadError = $state<string | null>(null);
	let thumbnails = $state<ThumbnailMap>({});
	let tab = $state<MediaTab>("all");
	let query = $state("");
	let view = $state<LibraryView>("grid");
	let sort = $state<LibrarySort>("recent");
	let selectedPath = $state<string | null>(null);
	const loadThumbnails = createThumbnailLoader();

	const items = $derived(toItems(recordings, exports));
	const counts = $derived(countByKind(items));
	const filtered = $derived(sortMedia(filterMedia(items, tab, query), sort));
	const displayed = $derived.by(() => {
		void view;
		return filtered.slice();
	});
	const status = $derived(
		libraryStatus({
			loading: isLoading,
			error: loadError,
			total: items.length,
			matches: filtered.length,
			query,
		}),
	);
	const selected = $derived(items.find((m) => m.entry.path === selectedPath) ?? null);

	async function refreshThumbnails(entries: RecordingEntry[]) {
		const next = await loadThumbnails(entries);
		if (next) thumbnails = next;
	}

	async function refresh() {
		isLoading = true;
		try {
			const [r, e] = await Promise.all([listRecasts(), listExports()]);
			recordings = r;
			exports = e;
			loadError = null;
			void refreshThumbnails([...r, ...e]);
		} catch (err) {
			loadError = String(err);
			toast.error(`Could not load media: ${err}`);
		} finally {
			isLoading = false;
		}
	}

	function restoreView() {
		view = safeStorage.get<LibraryView>(VIEW_KEY, view);
	}

	return {
		get isLoading() {
			return isLoading;
		},
		get loadError() {
			return loadError;
		},
		get thumbnails() {
			return thumbnails;
		},
		get counts() {
			return counts;
		},
		get displayed() {
			return displayed;
		},
		get status() {
			return status;
		},
		get totalSize() {
			return sumBytes([...recordings, ...exports]);
		},
		get selected() {
			return selected;
		},
		select(path: string) {
			selectedPath = selectedPath === path ? null : path;
		},
		isSelected(path: string) {
			return selectedPath === path;
		},
		get tab() {
			return tab;
		},
		set tab(value: MediaTab) {
			tab = value;
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
			safeStorage.set(VIEW_KEY, value);
		},
		get sort() {
			return sort;
		},
		set sort(value: LibrarySort) {
			sort = value;
		},
		refresh,
		restoreView,
	};
}

export type { MediaItem };
