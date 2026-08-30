// Resolvers never throw: a missing `ext:` id degrades to a built-in default and logs, so export and preview can't crash.

import { log } from "../log";
import { registry } from "./registry.svelte";
import {
	type CursorState,
	type CursorValue,
	type EasingValue,
	type Hotspot,
	isExtId,
	type SmoothingValue,
} from "./types";

/** Safe fallback background when an `ext:` background can't be resolved. */
const FALLBACK_BACKGROUND = "#111111";

/**
 * Resolve a stored `backgroundValue` to the string the render pipeline consumes.
 * Built-in values (hex, gradient, `asset:<id>`) pass through unchanged; only
 * `ext:` references hit the registry for the pack's hydrated absolute path.
 */
export function resolveBackgroundWireValue(value: string): string {
	if (!isExtId(value)) return value;
	const entry = registry.get("background", value);
	if (entry) return entry.value.wireValue;
	log.warn("registry", "background_missing", { id: value });
	return FALLBACK_BACKGROUND;
}

/**
 * Resolve a stored cursor style id to its sprite payload (svg + hotspots), or
 * null when unresolvable (callers fall back to the soft-dot cursor).
 */
export function resolveCursorSprite(id: string): CursorValue | null {
	const entry = registry.get("cursor", id);
	if (entry) return entry.value;
	if (isExtId(id)) {
		log.warn("registry", "cursor_missing", { id });
	}
	return null;
}

/** Pick a cursor sprite's SVG for a state, falling back rightPress/drag →
 *  press → rest so a style that only ships rest (+ press) never renders blank. */
export function cursorSpriteSvg(v: CursorValue, state: CursorState): string {
	switch (state) {
		case "rightPress":
			return v.rightPressedSvg ?? v.pressedSvg ?? v.svg;
		case "drag":
			return v.dragSvg ?? v.pressedSvg ?? v.svg;
		case "press":
			return v.pressedSvg ?? v.svg;
		default:
			return v.svg;
	}
}

/** Hotspot for a state, with the same fallback chain as {@link cursorSpriteSvg}. */
export function cursorSpriteHotspot(v: CursorValue, state: CursorState): Hotspot {
	switch (state) {
		case "rightPress":
			return v.rightPressedHotspot ?? v.pressedHotspot ?? v.hotspot;
		case "drag":
			return v.dragHotspot ?? v.pressedHotspot ?? v.hotspot;
		case "press":
			return v.pressedHotspot ?? v.hotspot;
		default:
			return v.hotspot;
	}
}

/** SVG data URLs cached per id+state so the preview overlay doesn't re-encode
 *  every frame. */
const cursorDataUrlCache = new Map<string, string>();

/**
 * Resolve a stored cursor id + state to a preview-ready SVG data URL, or null
 * when unresolvable (caller hides the overlay).
 */
export function resolveCursorDataUrl(id: string, state: CursorState): string | null {
	const key = `${id}:${state}`;
	const cached = cursorDataUrlCache.get(key);
	if (cached) return cached;
	const sprite = resolveCursorSprite(id);
	if (!sprite) return null;
	const svg = cursorSpriteSvg(sprite, state);
	const url = "data:image/svg+xml;utf8," + encodeURIComponent(svg.trim().replace(/\n\s*/g, " "));
	cursorDataUrlCache.set(key, url);
	return url;
}

/** Resolve a stored easing preset id to its {@link Easing} value, or null. */
export function resolveEasing(id: string): EasingValue["value"] | null {
	return registry.get("easing", id)?.value.value ?? null;
}

/** Resolve a stored smoothing preset id to its parameters, or null. */
export function resolveSmoothing(id: string): SmoothingValue | null {
	return registry.get("smoothing", id)?.value ?? null;
}
