/**
 * Pure helpers for the public share page, extracted from `+page.svelte` so the
 * scope mapping and `?t=` URL building stay isolated from the player/clipboard
 * wiring. (apps/web has no unit-test runner yet — plain pure functions verified
 * by svelte-check.)
 */

import { compactTime } from "$lib/share/format";

/** The three standard scopes the inline visibility toggle can write. */
export type LegacyVisibility = "public" | "team" | "private";

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
