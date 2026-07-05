/**
 * RecastCard pure helpers: folder ordering/indentation for the "Move to" submenu
 * and resolving a recast's assigned tag ids to full Tag objects.
 */

import type { Folder, Tag } from "$lib/dashboard/library.svelte";

/** Folders ordered by materialized path so nesting reads top-down. */
export function sortFoldersByPath(folders: Folder[]): Folder[] {
	return [...folders].sort((a, b) => a.path.localeCompare(b.path));
}

/** Nesting depth of a materialized path, used to drive submenu indentation. */
export function folderDepth(path: string): number {
	return Math.max(0, (path.match(/\//g)?.length ?? 1) - 2);
}

/** Full Tag objects for a recast's assigned tag ids, dropping any unknown ids. */
export function resolveAssignedTags(recastTags: string[], tags: Tag[]): Tag[] {
	return recastTags
		.map((id) => tags.find((t) => t.id === id))
		.filter((t): t is Tag => Boolean(t));
}
