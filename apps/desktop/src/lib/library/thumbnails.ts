/** Thumbnail loading + the immutable thumbnail-map updates for the listings. */

import { relativeDate } from "$lib/format/files";
import { generateThumbnails, type RecordingEntry } from "$lib/ipc";

export type ThumbnailMap = Record<string, string>;

/**
 * A single-frame thumbnail loader with a monotonic pass guard: a slower load
 * started earlier can't clobber a newer one. Each `load` resolves to the new
 * map, or `null` when a later load has already superseded it.
 */
export function createThumbnailLoader() {
	let pass = 0;
	return async function load(
		items: RecordingEntry[],
	): Promise<ThumbnailMap | null> {
		const current = ++pass;
		const settled = await Promise.allSettled(
			items.map(async (item) => {
				const frames = await generateThumbnails(item.path, 1);
				return [item.path, frames[0] ?? ""] as const;
			}),
		);
		if (current !== pass) return null;
		const next: ThumbnailMap = {};
		for (const r of settled) {
			if (r.status === "fulfilled" && r.value[1]) next[r.value[0]] = r.value[1];
		}
		return next;
	};
}

/** Re-key a thumbnail under a renamed path; unchanged if none was cached. */
export function renameThumbnail(
	map: ThumbnailMap,
	oldPath: string,
	newPath: string,
): ThumbnailMap {
	const existing = map[oldPath];
	if (!existing) return map;
	const { [oldPath]: _, ...rest } = map;
	return { ...rest, [newPath]: existing };
}

/** Drop one path's thumbnail; unchanged (same reference) if absent. */
export function removeThumbnail(map: ThumbnailMap, path: string): ThumbnailMap {
	if (!map[path]) return map;
	const { [path]: _, ...rest } = map;
	return rest;
}

/** Drop several paths' thumbnails in one immutable pass. */
export function removeThumbnails(
	map: ThumbnailMap,
	paths: Iterable<string>,
): ThumbnailMap {
	const next = { ...map };
	for (const p of paths) delete next[p];
	return next;
}

/** Listing date label: relative age, falling back to date+time past a week. */
export function libraryDate(unix: number): string {
	return relativeDate(unix, { withTime: true });
}
