/**
 * Multi-select state for the library listings: a reactive selection set plus
 * the toolbar/bulk-delete operations. The set is a `SvelteSet`, so component
 * reads through `has`/`count` stay reactive across this factory boundary.
 */

import type { RecordingEntry } from "$lib/ipc-types";
import { toast } from "@recast/ui/sonner";
import { SvelteSet } from "svelte/reactivity";

interface SelectionConfig {
	/** Singular listing noun for toast copy, e.g. `recording` / `export`. */
	noun: string;
	deleteFile: (path: string) => Promise<void>;
	/** Prune the caller's entries/thumbnails for the paths that were deleted. */
	onDeleted: (deleted: Set<string>) => void;
}

export function createSelection(config: SelectionConfig) {
	const selected = new SvelteSet<string>();
	let active = $state(false);

	function allSelected(filtered: RecordingEntry[]): boolean {
		return filtered.length > 0 && filtered.every((e) => selected.has(e.path));
	}

	function exit() {
		active = false;
		selected.clear();
	}

	function toggleMode() {
		if (active) exit();
		else active = true;
	}

	function toggle(path: string) {
		if (selected.has(path)) selected.delete(path);
		else selected.add(path);
	}

	function toggleAll(filtered: RecordingEntry[]) {
		if (allSelected(filtered)) selected.clear();
		else for (const e of filtered) selected.add(e.path);
	}

	async function bulkDelete() {
		const paths = [...selected];
		const results = await Promise.allSettled(
			paths.map((p) => config.deleteFile(p)),
		);
		const deleted = new Set<string>();
		results.forEach((r, i) => {
			if (r.status === "fulfilled") deleted.add(paths[i]);
		});
		config.onDeleted(deleted);
		const failed = paths.length - deleted.size;
		if (failed > 0) {
			toast.error(`Moved ${deleted.size} to trash · ${failed} failed`);
		} else {
			toast.success(
				`Moved ${deleted.size} ${config.noun}${deleted.size === 1 ? "" : "s"} to trash`,
			);
		}
		exit();
	}

	return {
		get selectMode() {
			return active;
		},
		get count() {
			return selected.size;
		},
		has: (path: string) => selected.has(path),
		allSelected,
		exit,
		toggleMode,
		toggle,
		toggleAll,
		bulkDelete,
	};
}
