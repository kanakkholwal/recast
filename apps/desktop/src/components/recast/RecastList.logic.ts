/** Pure search/grouping/keyboard-chord helpers for RecastList. */
import type { RecastAction, RecastListItem } from "./types";

/** `q` is expected pre-lowercased/trimmed by the caller. */
export function matches(item: RecastListItem, q: string): boolean {
	if (!q) return true;
	const haystack = [item.title, item.subtitle ?? "", ...(item.keywords ?? [])]
		.join(" ")
		.toLowerCase();
	return haystack.includes(q);
}

export function groupSections(
	filtered: RecastListItem[],
): { heading: string; items: RecastListItem[] }[] {
	const grouped = new Map<string, RecastListItem[]>();
	for (const item of filtered) {
		const key = item.section ?? "";
		if (!grouped.has(key)) grouped.set(key, []);
		grouped.get(key)!.push(item);
	}
	return Array.from(grouped.entries()).map(([heading, sectionItems]) => ({
		heading,
		items: sectionItems,
	}));
}

export function normalizeShortcut(s: string): string {
	return s.replace(/\s+/g, "").toLowerCase();
}

/** KeyboardEvent → normalized chord string (e.g. "⌘⌫"). Reads props only. */
export function chordFromEvent(e: KeyboardEvent): string {
	const parts: string[] = [];
	if (e.metaKey || e.ctrlKey) parts.push("⌘");
	if (e.shiftKey) parts.push("⇧");
	if (e.altKey) parts.push("⌥");
	const key = e.key;
	if (key === "Enter") parts.push("↵");
	else if (key === "Backspace") parts.push("⌫");
	else if (key === " ") parts.push("space");
	else parts.push(key.toUpperCase());
	return normalizeShortcut(parts.join(""));
}

export function findActionByChord(
	actions: RecastAction[] | undefined,
	chord: string,
): RecastAction | undefined {
	if (!actions) return undefined;
	return actions.find(
		(a) => a.shortcut && normalizeShortcut(a.shortcut) === chord,
	);
}

/** Primary dispatch: onSelect, else the first action. */
export function activate(item: RecastListItem): void {
	if (item.onSelect) item.onSelect();
	else if (item.actions && item.actions.length > 0) item.actions[0].onAction();
}
