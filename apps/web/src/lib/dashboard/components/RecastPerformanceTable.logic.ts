/**
 * Sorting model + column config for the RecastPerformanceTable. Pure so the
 * component only holds the current sort key/direction as reactive state.
 */

import { formatCount } from "$lib/dashboard/format";

export type Row = {
	id: string;
	title: string;
	posterUrl: string;
	views: number;
	avgWatch: number;
	completion: number;
	comments: number;
};

export type SortKey = "views" | "avgWatch" | "completion" | "comments";
export type SortDir = "asc" | "desc";

export interface SortState {
	key: SortKey;
	dir: SortDir;
}

export interface PerfColumn {
	key: SortKey;
	label: string;
	fmt: (r: Row) => string;
}

export const PERF_COLUMNS: PerfColumn[] = [
	{ key: "views", label: "Views", fmt: (r) => formatCount(r.views) },
	{ key: "avgWatch", label: "Avg watch", fmt: (r) => `${r.avgWatch}%` },
	{ key: "completion", label: "Completion", fmt: (r) => `${r.completion}%` },
	{ key: "comments", label: "Comments", fmt: (r) => formatCount(r.comments) },
];

/** Rows sorted by `key`/`dir`, capped at `limit`. Never mutates the input. */
export function sortRows(rows: Row[], key: SortKey, dir: SortDir, limit: number): Row[] {
	return [...rows]
		.sort((a, b) => (a[key] - b[key]) * (dir === "asc" ? 1 : -1))
		.slice(0, limit);
}

/** Sort state after clicking column `k`: flip direction if already active,
 *  otherwise switch to `k` sorted descending. */
export function nextSort(cur: SortState, k: SortKey): SortState {
	if (cur.key === k) return { key: k, dir: cur.dir === "asc" ? "desc" : "asc" };
	return { key: k, dir: "desc" };
}
