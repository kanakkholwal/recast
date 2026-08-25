import { listExports, listRecasts, type RecordingEntry } from "$lib/ipc";
import { recentSix } from "$lib/library/list";
import { createThumbnailLoader, type ThumbnailMap } from "$lib/library/thumbnails";
import { mergeRecents, type RecentItem } from "./home.logic";
import { toast } from "@recast/ui/sonner";

export type RecentFilter = "all" | "recording" | "export";

export function createHomeState() {
	let recordings = $state<RecordingEntry[]>([]);
	let exports = $state<RecordingEntry[]>([]);
	let isLoading = $state(true);
	let loadError = $state<string | null>(null);
	let thumbnails = $state<ThumbnailMap>({});
	let filter = $state<RecentFilter>("all");
	const loadThumbnails = createThumbnailLoader();

	const recents = $derived.by<RecentItem[]>(() => {
		if (filter === "recording") {
			return recentSix(recordings).map((entry) => ({ entry, kind: "recording" as const }));
		}
		if (filter === "export") {
			return recentSix(exports).map((entry) => ({ entry, kind: "export" as const }));
		}
		return mergeRecents(recordings, exports, 8);
	});

	async function refreshThumbnails(items: RecordingEntry[]) {
		const next = await loadThumbnails(items);
		if (next) thumbnails = next;
	}

	async function fetchAll() {
		isLoading = true;
		try {
			const [r, e] = await Promise.all([listRecasts(), listExports()]);
			recordings = recentSix(r);
			exports = recentSix(e);
			loadError = null;
			void refreshThumbnails([...recordings, ...exports]);
		} catch (err) {
			loadError = String(err);
			toast.error(`Could not load activity: ${err}`);
		} finally {
			isLoading = false;
		}
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
		get recents() {
			return recents;
		},
		get hasAny() {
			return recordings.length > 0 || exports.length > 0;
		},
		get filter() {
			return filter;
		},
		set filter(value: RecentFilter) {
			filter = value;
		},
		fetchAll,
	};
}
