/** Pure filter/sort + drag predicates for the recasts library page. */

import type { Recast, RecordingSource } from "./store.svelte";

export interface RecastFilter {
	query: string;
	activeFilter: RecordingSource | "all";
	/** Folder selection: "all", "root", or a folder id. */
	folder: string;
	tagIds: string[];
	sortKey: string;
}

function matchesFolder(r: Recast, folder: string): boolean {
	if (folder === "all") return true;
	if (folder === "root") return !r.folderId;
	return r.folderId === folder;
}

// Tag filter is OR — show recasts carrying ANY of the selected tags.
function matchesTags(r: Recast, tagIds: string[]): boolean {
	if (tagIds.length === 0) return true;
	return tagIds.some((id) => r.tags.includes(id));
}

export function filterAndSortRecasts(items: Recast[], f: RecastFilter): Recast[] {
	const q = f.query.trim().toLowerCase();
	const list = items.filter(
		(r) =>
			(f.activeFilter === "all" || r.source === f.activeFilter) &&
			matchesFolder(r, f.folder) &&
			matchesTags(r, f.tagIds) &&
			r.title.toLowerCase().includes(q),
	);
	return [...list].sort((a, b) => {
		switch (f.sortKey) {
			case "oldest":
				return a.createdAt - b.createdAt;
			case "name":
				return a.title.localeCompare(b.title);
			case "largest":
				return b.sizeBytes - a.sizeBytes;
			default:
				return b.createdAt - a.createdAt;
		}
	});
}

// Only external FILE drags: internal card-to-folder drags carry their own type and never trip the upload overlay.
export function isFileDrag(e: DragEvent): boolean {
	return Array.from(e.dataTransfer?.types ?? []).includes("Files");
}
