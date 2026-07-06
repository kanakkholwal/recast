/**
 * Pure helpers for the public share page, extracted from `+page.svelte` so the
 * scope mapping and `?t=` URL building stay isolated from the player/clipboard
 * wiring. (apps/web has no unit-test runner yet — plain pure functions verified
 * by svelte-check.)
 */

import type { RecastPlayerMarker } from "@recast/player";
import { compactTime } from "$lib/share/format";

/** The three standard scopes the inline visibility toggle can write. */
export type LegacyVisibility = "public" | "team" | "private";

/**
 * Project the comment thread onto the player scrubber — one marker per
 * time-anchored comment, coloured to match its author avatar so a viewer can
 * read the conversation off the timeline and jump straight to the moment.
 * Comments at 0:00 (the composer's default when nothing is playing) are
 * dropped: they're "general" notes, not tied to a spot on the video.
 */
export function buildCommentMarkers(
	comments: { id: string; authorName: string; atSeconds: number; body: string }[],
	hue: (seed: string) => number,
): RecastPlayerMarker[] {
	return comments
		.filter((c) => c.atSeconds > 0)
		.map((c) => ({
			id: c.id,
			time: c.atSeconds,
			label: `${c.authorName}: ${c.body.length > 60 ? `${c.body.slice(0, 60)}…` : c.body}`,
			kind: "comment" as const,
			color: `hsl(${hue(c.authorName)} 60% 45%)`,
		}));
}

/**
 * Collapse the full visibility enum to the toggle's three rows. `workspace` is
 * an alias of `team`; anything else (including `selected`/`private`) reads as
 * "private" for the row-active check.
 */
export function toLegacyVisibility(v: string): LegacyVisibility {
	if (v === "public") return "public";
	if (v === "team" || v === "workspace") return "team";
	return "private";
}

/**
 * Set (or clear at 0) the `?t=` deep-link param on `url` from a seconds offset,
 * returning the resulting href. Mutates the passed URL — callers hand it a fresh
 * `new URL(...)` each time.
 */
export function withTimeParam(url: URL, seconds: number): string {
	const t = compactTime(seconds);
	if (t) url.searchParams.set("t", t);
	else url.searchParams.delete("t");
	return url.toString();
}

/** The embeddable iframe snippet for a share URL. */
export function buildEmbedCode(url: string): string {
	return `<iframe src="${url}" width="640" height="360" frameborder="0" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen></iframe>`;
}
