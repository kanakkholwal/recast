/** Pure URL/query builders + sort math for the admin users list. */

const BASE = "/admin/users";

export type UsersFilterInput = {
	q: string;
	field: "email" | "name";
	role: string;
	status: string;
	sort: string;
	dir: string;
};

/**
 * Build the list URL from the filter bar. `reset` drops the current offset —
 * a filter change sends you back to page one.
 */
export function buildUsersQuery(
	filters: UsersFilterInput,
	opts: { limit: number; offset: number; reset: boolean },
): string {
	const sp = new URLSearchParams();
	if (filters.q.trim()) sp.set("q", filters.q.trim());
	sp.set("field", filters.field);
	if (filters.role !== "all") sp.set("role", filters.role);
	if (filters.status !== "all") sp.set("status", filters.status);
	sp.set("sort", filters.sort);
	sp.set("dir", filters.dir);
	sp.set("limit", String(opts.limit));
	if (!opts.reset) sp.set("offset", String(opts.offset));
	return `${BASE}?${sp.toString()}`;
}

/** Paginate relative to the current offset, preserving all other params. */
export function buildPageQuery(params: {
	search: string;
	offset: number;
	limit: number;
	delta: number;
}): string {
	const sp = new URLSearchParams(params.search);
	const newOffset = Math.max(0, params.offset + params.delta * params.limit);
	sp.set("offset", String(newOffset));
	return `${BASE}?${sp.toString()}`;
}

/** Same column flips asc/desc; a new column starts at desc. */
export function nextSortDir(
	currentSort: string,
	currentDir: string,
	field: string,
): "asc" | "desc" {
	const dir = currentSort === field ? currentDir : "desc";
	return dir === "desc" ? "asc" : "desc";
}

/** Sort URL for a header click — resets pagination, keeps other params. */
export function buildSortQuery(params: {
	search: string;
	currentSort: string;
	currentDir: string;
	field: string;
}): string {
	const sp = new URLSearchParams(params.search);
	sp.set("sort", params.field);
	sp.set("dir", nextSortDir(params.currentSort, params.currentDir, params.field));
	sp.delete("offset");
	return `${BASE}?${sp.toString()}`;
}

/** Arrow shown on the active sort column header. */
export function sortIndicator(
	currentSort: string,
	currentDir: string,
	field: string,
): string {
	if (currentSort !== field) return "";
	return currentDir === "asc" ? "↑" : "↓";
}
