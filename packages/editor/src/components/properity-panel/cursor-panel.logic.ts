/**
 * CursorPanel helpers: a safe data-URL for rendering (untrusted) cursor SVGs,
 * and the predicate behind the Animation section's "Reset" affordance.
 */

import type { EditorStore } from "../../stores/editor-store.svelte";

/** Default bounce window (ms): the "untouched" value for the reset check. */
const DEFAULT_BOUNCE_SPEED_MS = 220;

/**
 * Data-URL for a cursor SVG. Rendered via an `<img>` (not `{@html}`) so the SVG
 * loads in secure static mode, so no script executes from untrusted packs.
 */
export function svgSwatchUrl(svg: string): string {
	return "data:image/svg+xml;utf8," + encodeURIComponent(svg.trim().replace(/\n\s*/g, " "));
}

/** Whether any cursor-animation knob differs from its default (drives Reset). */
export function isCursorAnimTouched(settings: EditorStore["cursorSettings"]): boolean {
	return (
		settings.clickBounce !== 0 ||
		settings.sway !== 0 ||
		settings.motionBlur !== 0 ||
		settings.bounceSpeedMs !== DEFAULT_BOUNCE_SPEED_MS
	);
}
