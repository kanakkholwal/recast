import type { RecordingEntry } from "$lib/ipc";

export type RecentKind = "recording" | "export";

export interface RecentItem {
	entry: RecordingEntry;
	kind: RecentKind;
}

export function greeting(date: Date): string {
	const h = date.getHours();
	if (h < 12) return "Good morning";
	if (h < 18) return "Good afternoon";
	return "Good evening";
}

/** Newest recordings and exports interleaved by creation time, tagged by kind. */
export function mergeRecents(
	recordings: RecordingEntry[],
	exports: RecordingEntry[],
	limit: number,
): RecentItem[] {
	const tagged: RecentItem[] = [
		...recordings.map((entry) => ({ entry, kind: "recording" as const })),
		...exports.map((entry) => ({ entry, kind: "export" as const })),
	];
	tagged.sort((a, b) => b.entry.created - a.entry.created);
	return tagged.slice(0, limit);
}
