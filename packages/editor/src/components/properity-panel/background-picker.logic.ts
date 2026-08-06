/**
 * BackgroundPicker helpers: colour interpolation, gradient-stop sampling,
 * background `image` value classification, and per-mode value validation.
 */

import type { BackgroundType } from "../../stores/editor-store.svelte";

/** A gradient colour stop: `color` is a hex string, `pos` is 0..100. */
export interface GradientStop {
	color: string;
	pos: number;
}

/** Linearly interpolate two `#rrggbb` hex colours in sRGB. `f` is 0..1. */
export function lerpHex(c0: string, c1: string, f: number): string {
	const parse = (h: string): [number, number, number] => {
		const s = h.replace("#", "");
		return [parseInt(s.slice(0, 2), 16), parseInt(s.slice(2, 4), 16), parseInt(s.slice(4, 6), 16)];
	};
	const [r0, g0, b0] = parse(c0);
	const [r1, g1, b1] = parse(c1);
	const mix = (a: number, b: number) =>
		Math.round(a + (b - a) * f)
			.toString(16)
			.padStart(2, "0");
	return `#${mix(r0, r1)}${mix(g0, g1)}${mix(b0, b1)}`;
}

/**
 * Colour of a gradient at `pos` (0..100), interpolating surrounding stops in
 * sRGB, mirroring the renderer so an inserted stop is visually neutral. Stops
 * need not be pre-sorted.
 */
export function sampleStopColor(stops: GradientStop[], pos: number): string {
	const sorted = [...stops].sort((a, b) => a.pos - b.pos);
	if (pos <= sorted[0].pos) return sorted[0].color;
	const last = sorted[sorted.length - 1];
	if (pos >= last.pos) return last.color;
	for (let i = 0; i < sorted.length - 1; i++) {
		const a = sorted[i];
		const b = sorted[i + 1];
		if (pos >= a.pos && pos <= b.pos) {
			const f = (pos - a.pos) / Math.max(b.pos - a.pos, 1e-6);
			return lerpHex(a.color, b.color, f);
		}
	}
	return last.color;
}

/** Round + clamp a gradient-stop position into 0..100. */
export function clampStopPos(pos: number): number {
	return Math.round(Math.min(100, Math.max(0, pos)));
}

/** Pointer x → gradient-bar position (0..100, unclamped). `rect` needs only its
 *  left edge and width, so a plain object stands in for a DOMRect. */
export function posFromPointerX(clientX: number, rect: { left: number; width: number }): number {
	return ((clientX - rect.left) / Math.max(rect.width, 1)) * 100;
}

/**
 * The stop to drop into the widest gap between existing stops: positioned at the
 * gap midpoint with a colour sampled there, so the new handle lands somewhere
 * useful and looks visually neutral.
 */
export function insertStopInWidestGap(stops: GradientStop[]): GradientStop {
	const sorted = [...stops].sort((a, b) => a.pos - b.pos);
	let gapPos = 50;
	let widest = -1;
	for (let i = 0; i < sorted.length - 1; i++) {
		const gap = sorted[i + 1].pos - sorted[i].pos;
		if (gap > widest) {
			widest = gap;
			gapPos = Math.round((sorted[i].pos + sorted[i + 1].pos) / 2);
		}
	}
	return { color: sampleStopColor(stops, gapPos), pos: gapPos };
}

/** Non-image values that can linger in `backgroundValue` after a tab switch. */
function isNonImageValue(value: string): boolean {
	return value.includes("gradient(") || value.startsWith("#") || value.startsWith("asset:");
}

/** Sources that can be shown directly without going through `convertFileSrc`. */
function isDirectSrc(value: string): boolean {
	return (
		value.startsWith("data:") ||
		value.startsWith("http://") ||
		value.startsWith("https://") ||
		value.startsWith("asset://") ||
		value.startsWith("/wallpapers/")
	);
}

/**
 * Whether `value` is usable as a background image. Rejects gradient/colour/asset
 * leftovers (which would otherwise hit Tauri's asset protocol and log "file does
 * not exist").
 */
export function isValidImageValue(value: string): boolean {
	if (!value) return false;
	if (isNonImageValue(value)) return false;
	return (
		isDirectSrc(value) ||
		value.endsWith(".png") ||
		value.endsWith(".jpg") ||
		value.endsWith(".jpeg") ||
		value.endsWith(".webp")
	);
}

/**
 * Resolve a background `image` value to a `src`: `""` for non-image leftovers,
 * the value as-is for direct sources, else run through `resolve` (inject
 * `convertFileSrc` at the call site to keep this Tauri-free).
 */
export function imagePreviewSrc(value: string, resolve: (v: string) => string): string {
	if (!value) return "";
	if (isNonImageValue(value)) return "";
	if (isDirectSrc(value)) return value;
	return resolve(value);
}

/**
 * Whether `value` is a valid selection for a given background mode. Wallpaper
 * validity depends on the registry, so it's injected (`isRegisteredBackground`)
 * to keep this Tauri/registry-free.
 */
export function isValidValueForType(
	type: BackgroundType,
	value: string,
	isRegisteredBackground: (id: string) => boolean,
): boolean {
	switch (type) {
		case "wallpaper":
			// Any registered background id (built-in `asset:<id>` or an `ext:` pack).
			return isRegisteredBackground(value);
		case "color":
			return /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value);
		case "gradient":
			return value.includes("gradient(");
		case "image":
			return value.length > 0;
		default:
			return false;
	}
}

/**
 * Value to seed a mode with: keep the current value when it's valid for that
 * mode, else fall back to the mode's default.
 */
export function selectionValueForType(
	type: BackgroundType,
	currentValue: string,
	defaults: Record<BackgroundType, string>,
	isRegisteredBackground: (id: string) => boolean,
): string {
	return isValidValueForType(type, currentValue, isRegisteredBackground)
		? currentValue
		: defaults[type];
}
