import { toast } from "@recast/ui/sonner";
import { listExports, listRecasts, type RecordingEntry } from "$lib/ipc";
import { sortEntries } from "$lib/library/list";
import { createThumbnailLoader, type ThumbnailMap } from "$lib/library/thumbnails";
import { mergeRecents, type RecentItem } from "./home.logic";

const RECENTS_LIMIT = 8;

export function createHomeState() {
	let recordings = $state<RecordingEntry[]>([]);
	let exports = $state<RecordingEntry[]>([]);
	let isLoading = $state(true);
	let loadError = $state<string | null>(null);
	let thumbnails = $state<ThumbnailMap>({});
	const loadThumbnails = createThumbnailLoader();

	const recents = $derived<RecentItem[]>(mergeRecents(recordings, exports, RECENTS_LIMIT));

	async function refreshThumbnails(items: RecordingEntry[]) {
		const next = await loadThumbnails(items);
		if (next) thumbnails = next;
	}

	async function fetchAll() {
		isLoading = true;
		try {
			const [r, e] = await Promise.all([listRecasts(), listExports()]);
			recordings = sortEntries(r, "recent").slice(0, RECENTS_LIMIT);
			exports = sortEntries(e, "recent").slice(0, RECENTS_LIMIT);
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
		fetchAll,
	};
}
