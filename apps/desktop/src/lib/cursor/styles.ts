/**
 * Curated cursor sprite library. Each style is an inline SVG so we can
 * recolour and resample at any DPI without bundling pixel assets.
 *
 * Coordinate system: every sprite is authored at 64×64 with the *click
 * hotspot* at `hotspot` (in sprite-space px). The preview overlay applies
 * `transform: translate(-hotspotX, -hotspotY)` so the cursor's tip lands on
 * the captured pointer position regardless of which sprite is selected.
 *
 * `dot` is the historical soft-circle path, drawn by the WebGL2 shader and
 * the Rust export overlay. `macos` adds an Apple-style cursor with two
 * sprites: the arrow shown at rest, and the link-pointing hand swapped in
 * while the captured cursor is mid-click. Per-state lookup happens via
 * `cursorStyleDataUrl(id, "press" | "rest")`.
 */

import type { CursorStyleId } from "$lib/stores/editor-store.svelte";

export interface CursorStyle {
	id: CursorStyleId;
	label: string;
	/** Short blurb shown under the swatch in the panel. */
	description: string;
	/** Authored at 64×64 with the click hotspot at `hotspot`. */
	svg: string;
	/** Optional pressed-state sprite swapped in while the captured cursor
	 *  is mid-click. When omitted the rest sprite is reused. */
	pressedSvg?: string;
	hotspot: { x: number; y: number };
	pressedHotspot?: { x: number; y: number };
}

// All sprites are 64×64 viewBox. The macOS shapes are scaled up from the
// classic 16×16 system cursors so every stroke reads cleanly at the 32–64 px
// rendered scale users see in playback.

// Apple's resting pointer: a tilted arrow with a black fill, white outline,
// and a 1 px gap on the inside corner. The path is anchored at (8, 6) so
// `transform: translate(-12.5%, -9.4%)` keeps the tip on the pointer sample.
const MACOS_ARROW = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <path
    d="M8 6 L8 50.4 L19.6 39.2 L25.6 53.6 L31.6 51 L25.8 36.6 L42 36.6 Z"
    fill="#000" stroke="#fff" stroke-width="3.2" stroke-linejoin="round" stroke-linecap="round"/>
  <path
    d="M8 6 L8 50.4 L19.6 39.2 L25.6 53.6 L31.6 51 L25.8 36.6 L42 36.6 Z"
    fill="#000"/>
</svg>`;

// Apple's link pointer: a flat hand with the index finger raised. Hotspot
// sits at the tip of the index (12, 4 in 64×64 space), matching how macOS
// reports the click coordinate from the system hand cursor.
const MACOS_POINTER = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <path
    d="
      M12 4
      C 14 4, 16 5.6, 16 8
      L 16 28
      L 18 28
      L 18 22
      C 18 20.4, 19.6 19, 21.2 19
      C 22.8 19, 24.4 20.4, 24.4 22
      L 24.4 28
      L 26.4 28
      L 26.4 24
      C 26.4 22.4, 28 21, 29.6 21
      C 31.2 21, 32.8 22.4, 32.8 24
      L 32.8 28
      L 34.8 28
      L 34.8 26
      C 34.8 24.4, 36.4 23, 38 23
      C 39.6 23, 41.2 24.4, 41.2 26
      L 41.2 42
      C 41.2 50, 36.4 56, 28 56
      L 24 56
      C 17.6 56, 12 50, 12 44
      L 12 36
      L 9.6 32
      C 8.4 30.4, 8.8 28, 10.4 26.8
      C 12 25.6, 14 26, 15.2 27.6
      L 16 28.6
      L 16 8
      C 16 5.6, 13 4, 12 4
      Z"
    fill="#000" stroke="#fff" stroke-width="3" stroke-linejoin="round" stroke-linecap="round"/>
  <path
    d="
      M12 4
      C 14 4, 16 5.6, 16 8
      L 16 28
      L 18 28
      L 18 22
      C 18 20.4, 19.6 19, 21.2 19
      C 22.8 19, 24.4 20.4, 24.4 22
      L 24.4 28
      L 26.4 28
      L 26.4 24
      C 26.4 22.4, 28 21, 29.6 21
      C 31.2 21, 32.8 22.4, 32.8 24
      L 32.8 28
      L 34.8 28
      L 34.8 26
      C 34.8 24.4, 36.4 23, 38 23
      C 39.6 23, 41.2 24.4, 41.2 26
      L 41.2 42
      C 41.2 50, 36.4 56, 28 56
      L 24 56
      C 17.6 56, 12 50, 12 44
      L 12 36
      L 9.6 32
      C 8.4 30.4, 8.8 28, 10.4 26.8
      C 12 25.6, 14 26, 15.2 27.6
      L 16 28.6
      Z"
    fill="#000"/>
</svg>`;

const DOT_SWATCH = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <circle cx="32" cy="32" r="9" fill="#ffffff"/>
  <circle cx="32" cy="32" r="11" fill="none" stroke="#fff" stroke-opacity="0.35" stroke-width="3"/>
</svg>`;

export const CURSOR_STYLES: CursorStyle[] = [
	{
		id: "dot",
		label: "Soft dot",
		description: "Default — used for both preview and export.",
		// The actual `dot` cursor is drawn by the WebGL2 shader; this SVG is
		// only the picker swatch.
		svg: DOT_SWATCH,
		hotspot: { x: 32, y: 32 },
	},
	{
		id: "macos",
		label: "macOS",
		description: "Apple-style arrow that turns into the link pointer on click.",
		svg: MACOS_ARROW,
		pressedSvg: MACOS_POINTER,
		hotspot: { x: 8, y: 6 },
		pressedHotspot: { x: 12, y: 4 },
	},
];

export function getCursorStyle(id: CursorStyleId): CursorStyle {
	return (
		CURSOR_STYLES.find((s) => s.id === id) ??
		CURSOR_STYLES[0]
	);
}

export type CursorStyleState = "rest" | "press";

export function cursorStyleHotspot(
	id: CursorStyleId,
	state: CursorStyleState = "rest",
): { x: number; y: number } {
	const style = getCursorStyle(id);
	if (state === "press" && style.pressedHotspot) return style.pressedHotspot;
	return style.hotspot;
}

/** Cached `data:image/svg+xml,…` URLs (one per id+state) so the `<img>`
 *  element in the overlay layer doesn't re-encode on every frame. */
const dataUrlCache = new Map<string, string>();
export function cursorStyleDataUrl(
	id: CursorStyleId,
	state: CursorStyleState = "rest",
): string {
	const key = `${id}:${state}`;
	const cached = dataUrlCache.get(key);
	if (cached) return cached;
	const style = getCursorStyle(id);
	const svg =
		state === "press" && style.pressedSvg ? style.pressedSvg : style.svg;
	const url =
		"data:image/svg+xml;utf8," +
		encodeURIComponent(svg.trim().replace(/\n\s*/g, " "));
	dataUrlCache.set(key, url);
	return url;
}
