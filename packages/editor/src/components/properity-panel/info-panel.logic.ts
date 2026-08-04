/** InfoPanel helpers: relative-time labels, path basename, and annotation-kind counts. */

import type { Annotation } from "$lib/stores/editor-store.svelte";

/**
 * Chat-style relative time vs a `current` epoch (ms): "just now", "5s", "3 min",
 * "2 hr", "4 days", prefixed "in "/suffixed " ago". Floors at each cutoff so the
 * readout doesn't bounce by a unit each tick.
 */
export function formatRelative(ts: number | null, current: number): string {
	if (!ts) return "Never";
	const diffMs = current - ts;
	const future = diffMs < 0;
	const seconds = Math.floor(Math.abs(diffMs) / 1000);
	const minutes = Math.floor(seconds / 60);
	const hours = Math.floor(minutes / 60);
	const days = Math.floor(hours / 24);
	let label: string;
	if (seconds < 5) label = "just now";
	else if (seconds < 60) label = `${seconds}s`;
	else if (minutes < 60) label = `${minutes} min`;
	else if (hours < 24) label = `${hours} hr`;
	else label = `${days} day${days === 1 ? "" : "s"}`;
	if (label === "just now") return label;
	return future ? `in ${label}` : `${label} ago`;
}

/** Last path segment (handles both separators); "—" when empty. */
export function basename(path: string): string {
	if (!path) return "—";
	const sep = path.includes("\\") ? "\\" : "/";
	const last = path.split(sep).filter(Boolean).pop() ?? path;
	return last;
}

/** Count annotations by kind; seeds every kind to 0 so the readout row doesn't shift. */
export function countByKind(annotations: Annotation[]): Record<string, number> {
	const out: Record<string, number> = {
		rect: 0,
		ellipse: 0,
		arrow: 0,
		text: 0,
		image: 0,
	};
	for (const a of annotations) out[a.kind.kind] = (out[a.kind.kind] ?? 0) + 1;
	return out;
}
