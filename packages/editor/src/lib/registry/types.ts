/**
 * Asset registry shared types: Tier 0 of the extensions architecture, one
 * addressable catalog for the editor's visual assets (cursors, backgrounds,
 * gradients, colours, easing, smoothing presets).
 *
 * Storage ids: built-ins keep their exact legacy form (bare local id, or a
 * literal hex/gradient for inline-stored kinds) so old project files resolve
 * with no migration. Extension entries are `ext:<extId>:<localId>` so they
 * never collide with built-ins and degrade gracefully when the pack is removed.
 */

import type { Easing } from "../easing/cubic-bezier";
import type { CaptionAnimation } from "@recast/captions";

export type AssetKind =
	| "cursor"
	| "background"
	| "gradient"
	| "color"
	| "easing"
	| "smoothing"
	| "captionPreset";

export type Source = { kind: "builtin" } | { kind: "extension"; extId: string };

/** Hotspot in sprite-space pixels (sprites are authored at 64×64). */
export interface Hotspot {
	x: number;
	y: number;
}

/** Render-time cursor states. `press` is the primary (left) click; `rightPress`
 *  and `drag` fall back to `press` → rest when a style doesn't ship them. */
export type CursorState = "rest" | "press" | "rightPress" | "drag";

/** Per-kind `value` payloads carried by a {@link RegistryEntry}. */
export interface CursorValue {
	/** Raw SVG string (rest state). */
	svg: string;
	/** Optional pressed-state SVG swapped in mid-click (left button). */
	pressedSvg?: string;
	/** Optional right-click SVG. Falls back to {@link pressedSvg} → {@link svg}. */
	rightPressedSvg?: string;
	/** Optional drag SVG (button held while moving). Falls back to
	 *  {@link pressedSvg} → {@link svg}. */
	dragSvg?: string;
	hotspot: Hotspot;
	pressedHotspot?: Hotspot;
	rightPressedHotspot?: Hotspot;
	dragHotspot?: Hotspot;
}
export interface BackgroundValue {
	/** The string the render pipeline consumes for `backgroundValue`:
	 *  `asset:<id>` (built-in wallpaper), an absolute file path (extension
	 *  wallpaper/image), a hex, or a CSS gradient. */
	wireValue: string;
}
export interface GradientValue {
	/** CSS `linear-gradient(...)` string: the source of truth both renderers parse. */
	value: string;
}
export interface ColorValue {
	/** Hex colour. */
	value: string;
}
export interface EasingValue {
	value: Easing;
}
export interface SmoothingValue {
	smoothing: number;
	snapToClicks: boolean;
	snapWindowMs: number;
}
/** A caption look: the visual half of `CaptionStyle` (everything except
 *  `enabled`). Applied wholesale to the editor's caption style; built-ins ship
 *  a few themes and extension packs can contribute more. Kept structurally in
 *  sync with `CaptionStyle` in the editor store (this module can't import the
 *  store without a cycle). */
export interface CaptionPresetValue {
	fontFamily: string;
	fontWeight: number;
	fontSizePct: number;
	position: "top" | "center" | "bottom";
	align: "left" | "center" | "right";
	offsetPct: number;
	color: string;
	uppercase: boolean;
	letterSpacing: number;
	/** Unspoken-word colour for progressive highlight. */
	mutedColor: string;
	background: "none" | "soft" | "box";
	backgroundColor: string;
	backgroundOpacity: number;
	/** Pill padding / radius / line height, in em of the caption font size
	 *  (radius clamps to a stadium at render). */
	boxPaddingXEm: number;
	boxPaddingYEm: number;
	boxRadiusEm: number;
	lineHeight: number;
	outlineWidth: number;
	outlineColor: string;
	maxLines: number;
	/** Greedy line-break width, in characters. */
	maxCharsPerLine: number;
	/** Word-by-word animation. Absent = static. */
	animation?: CaptionAnimation;
}

export type RegistryValueFor<K extends AssetKind> = K extends "cursor"
	? CursorValue
	: K extends "background"
		? BackgroundValue
		: K extends "gradient"
			? GradientValue
			: K extends "color"
				? ColorValue
				: K extends "easing"
					? EasingValue
					: K extends "smoothing"
						? SmoothingValue
						: K extends "captionPreset"
							? CaptionPresetValue
							: never;

export interface RegistryEntry<K extends AssetKind = AssetKind> {
	/** Storage id; see module docs. Unique within a kind. */
	id: string;
	kind: K;
	label: string;
	source: Source;
	value: RegistryValueFor<K>;
	/** Optional secondary line shown under the swatch in pickers. */
	description?: string;
	/** Asset id (extension manifest-local) whose hydrated thumbnail represents
	 *  this entry in the picker. Built-ins typically omit this. */
	thumbAssetId?: string;
	/** Ready-to-use WebView thumbnail URL (`convertFileSrc` of a hydrated
	 *  extension asset). Set for extension entries; built-ins instead resolve a
	 *  thumbnail from {@link thumbAssetId} via the assets store. */
	thumbUrl?: string;
}

/** Prefix marking an extension-contributed storage id. */
export const EXT_PREFIX = "ext:";

/** Build the storage id for an extension-contributed entry. */
export function extEntryId(extId: string, localId: string): string {
	return `${EXT_PREFIX}${extId}:${localId}`;
}

/** True when a stored id refers to an extension-contributed entry. */
export function isExtId(id: string): boolean {
	return id.startsWith(EXT_PREFIX);
}

/** Parse `ext:<extId>:<localId>` → `{ extId, localId }`, or null if not an
 *  extension id (or malformed). `localId` may itself contain colons. */
export function parseExtId(id: string): { extId: string; localId: string } | null {
	if (!isExtId(id)) return null;
	const rest = id.slice(EXT_PREFIX.length);
	const sep = rest.indexOf(":");
	if (sep <= 0 || sep >= rest.length - 1) return null;
	return { extId: rest.slice(0, sep), localId: rest.slice(sep + 1) };
}
