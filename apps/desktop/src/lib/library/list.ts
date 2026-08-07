/** Search, sort, and size maths for the recordings/exports/activity listings. */

import { getExtension } from "@recast/editor/lib/format/files";
import type { RecordingEntry } from "@recast/editor/lib/wire-types";

export type LibrarySort = "recent" | "name" | "size";

/**
 * Case-insensitive filename search. `matchExtension` also matches the file's
 * extension (exports search by type; recordings don't).
 */
export function filterEntries(
	entries: RecordingEntry[],
	query: string,
	opts: { matchExtension?: boolean } = {},
): RecordingEntry[] {
	const q = query.trim().toLowerCase();
	if (!q) return entries.slice();
	return entries.filter(
		(e) =>
			e.filename.toLowerCase().includes(q) ||
			(!!opts.matchExtension && getExtension(e.filename).toLowerCase().includes(q)),
	);
}

/** Sorted copy: newest-first, A→Z, or largest-first. */
export function sortEntries(entries: RecordingEntry[], sort: LibrarySort): RecordingEntry[] {
	const list = entries.slice();
	if (sort === "recent") list.sort((a, b) => b.created - a.created);
	else if (sort === "name") list.sort((a, b) => a.filename.localeCompare(b.filename));
	else if (sort === "size") list.sort((a, b) => b.sizeBytes - a.sizeBytes);
	return list;
}

/** The six most recent entries: the home page's activity strips. */
export function recentSix(entries: RecordingEntry[]): RecordingEntry[] {
	return sortEntries(entries, "recent").slice(0, 6);
}

export function sumBytes(entries: RecordingEntry[]): number {
	return entries.reduce((sum, e) => sum + e.sizeBytes, 0);
}
