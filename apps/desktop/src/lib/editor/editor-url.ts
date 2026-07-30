import { PANEL_TABS, type PanelTab } from "$lib/editor/panel-tabs";

/**
 * Editor view state carried in the URL, so a reload or a shared link lands on
 * the same section with the same chrome showing.
 *
 * The URL is authoritative when a param is present. Absent, the caller's own
 * default applies — which for the sidebar and timeline is the remembered
 * localStorage preference, so "my usual layout" still governs a fresh open
 * while an explicit link still wins.
 */
export const PANEL_PARAM = "tab";
export const SIDEBAR_PARAM = "sidebar";
export const TIMELINE_PARAM = "timeline";

/**
 * Validate a raw `?tab=` value. Returns null for anything unknown so a stale or
 * hand-edited URL falls back to the store's default rather than selecting a tab
 * that renders nothing.
 *
 * `dev` is a dev-build-only tab, so it is rejected unless the caller says the
 * build has it — otherwise a production URL could select a tab with no trigger
 * in the rail, leaving the panel showing no selection at all.
 */
export function parsePanelTab(raw: string | null | undefined, allowDev = false): PanelTab | null {
	if (!raw) return null;
	const value = raw.trim().toLowerCase();
	const match = PANEL_TABS.find((t) => t === value);
	if (!match) return null;
	if (match === "dev" && !allowDev) return null;
	return match;
}

const TRUE_VALUES = new Set(["1", "true", "yes", "on"]);
const FALSE_VALUES = new Set(["0", "false", "no", "off"]);

/** Boolean param, or null when absent/unrecognised so the caller keeps its own
 *  default instead of being forced to guess. Tolerant of hand-edited URLs. */
export function parseBoolParam(raw: string | null | undefined): boolean | null {
	if (raw === null || raw === undefined) return null;
	const value = raw.trim().toLowerCase();
	if (TRUE_VALUES.has(value)) return true;
	if (FALSE_VALUES.has(value)) return false;
	return null;
}

export function boolParam(value: boolean): string {
	return value ? "1" : "0";
}

/**
 * `url` with every entry applied, or null when it already says all of them.
 *
 * One call for the whole view rather than one per param: separate writers would
 * each build from a `page.url` the others hadn't updated yet, so the last
 * `replaceState` of a flush would silently drop its siblings' changes. Returning
 * null is also what keeps the writer from looping against the reader.
 */
export function withEditorParams(url: URL, params: Record<string, string>): URL | null {
	const entries = Object.entries(params);
	if (entries.every(([k, v]) => url.searchParams.get(k) === v)) return null;
	const next = new URL(url);
	for (const [k, v] of entries) next.searchParams.set(k, v);
	return next;
}
