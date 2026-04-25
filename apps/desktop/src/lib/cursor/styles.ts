/**
 * Curated cursor sprite library. Each style is an inline SVG so we can
 * recolour and resample at any DPI without bundling pixel assets.
 *
 * Coordinate system: every sprite is authored at 64×64 with the *click
 * hotspot* at `hotspot` (in sprite-space px). The preview overlay applies
 * `transform: translate(-hotspotX, -hotspotY)` so the cursor's tip lands on
 * the captured pointer position regardless of which sprite is selected.
 *
 * `dot` is the historical soft-circle path and stays handled by the WebGL2
 * shader / Rust export overlay. The other styles render via
 * `CursorOverlayLayer.svelte` (preview-only for now; export currently falls
 * back to the soft circle until the same PNG raster is plumbed through the
 * existing image-cache pipeline in `cursor_export.rs`).
 */

import type { CursorStyleId } from "$lib/stores/editor-store.svelte";

export interface CursorStyle {
	id: CursorStyleId;
	label: string;
	/** Short blurb shown under the swatch in the panel. */
	description: string;
	/** Authored at 64×64 with the click hotspot at `hotspot`. */
	svg: string;
	hotspot: { x: number; y: number };
}

// All sprites are 64×64 viewBox. Stroke widths are tuned so they read at
// 32px on screen (the typical post-`size` rendered scale).

const ARROW_SYSTEM = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <defs>
    <filter id="cs-shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur in="SourceAlpha" stdDeviation="0.8"/>
      <feOffset dx="0" dy="0.6" result="o"/>
      <feComponentTransfer><feFuncA type="linear" slope="0.45"/></feComponentTransfer>
      <feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
  </defs>
  <path
    d="M8 6 L8 50 L19 39 L26 56 L33 53 L26 36 L42 36 Z"
    fill="#ffffff" stroke="#111" stroke-width="2.5" stroke-linejoin="round"
    filter="url(#cs-shadow)"/>
</svg>`;

const ARROW_MINIMAL = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <path
    d="M10 8 L10 46 L21 36 L28 50 L34 47 L27 33 L42 33 Z"
    fill="#ffffff" stroke="#111" stroke-width="1.5" stroke-linejoin="round"/>
</svg>`;

const ARROW_FAT = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <defs>
    <linearGradient id="cs-fat" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#ffffff"/>
      <stop offset="1" stop-color="#dddddd"/>
    </linearGradient>
  </defs>
  <path
    d="M6 4 L6 54 L20 42 L28 60 L38 56 L30 38 L48 38 Z"
    fill="url(#cs-fat)" stroke="#111" stroke-width="3.5" stroke-linejoin="round"/>
</svg>`;

const TARGET_RETICLE = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <circle cx="32" cy="32" r="14" fill="none" stroke="#ffffff" stroke-width="2.5"/>
  <circle cx="32" cy="32" r="14" fill="none" stroke="#111" stroke-width="1" stroke-dasharray="3 3"/>
  <line x1="32" y1="10" x2="32" y2="22" stroke="#ffffff" stroke-width="2.5"/>
  <line x1="32" y1="42" x2="32" y2="54" stroke="#ffffff" stroke-width="2.5"/>
  <line x1="10" y1="32" x2="22" y2="32" stroke="#ffffff" stroke-width="2.5"/>
  <line x1="42" y1="32" x2="54" y2="32" stroke="#ffffff" stroke-width="2.5"/>
  <circle cx="32" cy="32" r="2.5" fill="#ffffff" stroke="#111" stroke-width="1"/>
</svg>`;

export const CURSOR_STYLES: CursorStyle[] = [
	{
		id: "dot",
		label: "Soft dot",
		description: "Default — used for both preview and export.",
		// Placeholder SVG just for the picker swatch; the real cursor for
		// `dot` is drawn by the WebGL2 shader, not this overlay.
		svg: `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
        <circle cx="32" cy="32" r="9" fill="#ffffff"/>
        <circle cx="32" cy="32" r="11" fill="none" stroke="#fff" stroke-opacity="0.35" stroke-width="3"/>
      </svg>`,
		hotspot: { x: 32, y: 32 },
	},
	{
		id: "system",
		label: "System pointer",
		description: "Crisp white arrow with a black stroke.",
		svg: ARROW_SYSTEM,
		hotspot: { x: 8, y: 6 },
	},
	{
		id: "minimal",
		label: "Minimal arrow",
		description: "Thin-stroke arrow for low-contrast UIs.",
		svg: ARROW_MINIMAL,
		hotspot: { x: 10, y: 8 },
	},
	{
		id: "fat-arrow",
		label: "Bold arrow",
		description: "High-visibility arrow for tutorials.",
		svg: ARROW_FAT,
		hotspot: { x: 6, y: 4 },
	},
	{
		id: "target",
		label: "Reticle",
		description: "Crosshair for precision callouts.",
		svg: TARGET_RETICLE,
		hotspot: { x: 32, y: 32 },
	},
];

export function getCursorStyle(id: CursorStyleId): CursorStyle {
	return (
		CURSOR_STYLES.find((s) => s.id === id) ??
		CURSOR_STYLES[0]
	);
}

/** Cached `data:image/svg+xml,…` URLs (one per id) so the `<img>` element in
 *  the overlay layer doesn't re-encode on every frame. */
const dataUrlCache = new Map<CursorStyleId, string>();
export function cursorStyleDataUrl(id: CursorStyleId): string {
	const cached = dataUrlCache.get(id);
	if (cached) return cached;
	const style = getCursorStyle(id);
	const url =
		"data:image/svg+xml;utf8," +
		encodeURIComponent(style.svg.trim().replace(/\n\s*/g, " "));
	dataUrlCache.set(id, url);
	return url;
}
