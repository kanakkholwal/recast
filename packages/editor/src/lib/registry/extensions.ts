/**
 * Bridge installed asset-packs → the asset registry.
 *
 * Given a hydrated {@link InstalledExtension} (manifest contributions + resolved
 * on-disk asset paths), build {@link RegistryEntry} objects and register them
 * under `ext:<extId>:<localId>` ids. Cursor SVGs are read as text through the
 * Tauri asset protocol (the render/preview paths need the SVG string, not a
 * file path, because neither has an SVG decoder on the Rust side). Background
 * entries resolve to the pack's absolute image path (the render pipeline and
 * `convertFileSrc` both accept it).
 *
 * Kept OUT of `lib/registry/index.ts` so the built-ins side-effect path never
 * pulls in Tauri APIs; callers import this module directly.
 */

import { tryGetEditorServices } from "../editor/services";

/** Identity on hosts whose refs are already loadable (web object URLs). */
const resolveRef = (r: string) => tryGetEditorServices()?.resolveAssetUrl(r) ?? r;

import { DEFAULT_CAPTION_STYLE } from "@recast/captions";
import { log } from "../log";
import type { InstalledExtension } from "../wire-types";
import { registry } from "./registry.svelte";
import { extEntryId, type RegistryEntry } from "./types";

type AssetMap = Map<string, { path: string | null; thumbPath: string | null }>;

function assetMap(ext: InstalledExtension): AssetMap {
	const m: AssetMap = new Map();
	for (const a of ext.assets) {
		m.set(a.id, { path: a.path, thumbPath: a.thumbPath });
	}
	return m;
}

/** Fetch a hydrated SVG asset's text via the Tauri asset protocol. */
async function loadSvg(path: string): Promise<string | null> {
	try {
		const res = await fetch(resolveRef(path));
		if (!res.ok) return null;
		return await res.text();
	} catch (err) {
		log.warn("registry", "ext_svg_load_failed", { err: String(err) });
		return null;
	}
}

/**
 * Register every contribution of one installed, enabled pack. Disabled packs
 * are skipped (their entries should not appear in pickers). Returns the number
 * of entries registered.
 */
export async function registerExtension(ext: InstalledExtension): Promise<number> {
	// Start from a clean slate so re-registration (toggle or reinstall) never leaves stale entries behind.
	registry.unregisterExtension(ext.manifest.id);
	if (!ext.enabled) return 0;

	const extId = ext.manifest.id;
	const assets = assetMap(ext);
	const contributes = ext.manifest.contributes ?? {};
	const entries: RegistryEntry[] = [];

	// Cursors need the SVG text; load rest (+ optional press) concurrently.
	for (const c of contributes.cursors ?? []) {
		const restPath = assets.get(c.rest)?.path;
		if (!restPath) {
			log.warn("registry", "ext_cursor_missing_asset", { extId, id: c.id });
			continue;
		}
		// Resolve an optional manifest-local asset id to its SVG text, or null.
		const loadOptional = (assetId: string | undefined) => {
			const path = assetId ? assets.get(assetId)?.path : undefined;
			return path ? loadSvg(path) : Promise.resolve(null);
		};

		const [svg, pressedSvg, rightPressedSvg, dragSvg] = await Promise.all([
			loadSvg(restPath),
			loadOptional(c.press),
			loadOptional(c.rightPress),
			loadOptional(c.drag),
		]);
		if (!svg) {
			log.warn("registry", "ext_cursor_svg_failed", { extId, id: c.id });
			continue;
		}
		entries.push({
			id: extEntryId(extId, c.id),
			kind: "cursor",
			label: c.label,
			description: c.description,
			source: { kind: "extension", extId },
			value: {
				svg,
				pressedSvg: pressedSvg ?? undefined,
				rightPressedSvg: rightPressedSvg ?? undefined,
				dragSvg: dragSvg ?? undefined,
				hotspot: c.hotspot,
				pressedHotspot: c.pressedHotspot,
				rightPressedHotspot: c.rightPressedHotspot,
				dragHotspot: c.dragHotspot,
			},
		});
	}

	// Backgrounds: wireValue is the pack's absolute image path.
	for (const b of contributes.backgrounds ?? []) {
		const mainAsset = assets.get(b.asset);
		const full = mainAsset?.path;
		if (!full) {
			log.warn("registry", "ext_background_missing_asset", { extId, id: b.id });
			continue;
		}
		// Prefer an explicit thumb, then the hydrated per-asset thumbnail, and only then decode the full-res image.
		const thumbPath = (b.thumb && assets.get(b.thumb)?.path) || mainAsset.thumbPath || full;
		entries.push({
			id: extEntryId(extId, b.id),
			kind: "background",
			label: b.label,
			source: { kind: "extension", extId },
			thumbUrl: resolveRef(thumbPath),
			value: { wireValue: full },
		});
	}

	for (const g of contributes.gradients ?? []) {
		entries.push({
			id: extEntryId(extId, g.id),
			kind: "gradient",
			label: g.label,
			source: { kind: "extension", extId },
			value: { value: g.value },
		});
	}

	for (const col of contributes.colors ?? []) {
		entries.push({
			id: extEntryId(extId, col.id),
			kind: "color",
			label: col.label,
			source: { kind: "extension", extId },
			value: { value: col.value },
		});
	}

	for (const e of contributes.easings ?? []) {
		entries.push({
			id: extEntryId(extId, e.id),
			kind: "easing",
			label: e.label,
			source: { kind: "extension", extId },
			value: { value: e.value },
		});
	}

	for (const s of contributes.smoothings ?? []) {
		entries.push({
			id: extEntryId(extId, s.id),
			kind: "smoothing",
			label: s.label,
			source: { kind: "extension", extId },
			value: {
				smoothing: s.smoothing,
				snapToClicks: s.snapToClicks,
				snapWindowMs: s.snapWindowMs,
			},
		});
	}

	// Caption themes carry their whole style payload in the manifest, so contribution fields map straight across.
	for (const p of contributes.captionPresets ?? []) {
		entries.push({
			id: extEntryId(extId, p.id),
			kind: "captionPreset",
			label: p.label,
			description: p.description,
			source: { kind: "extension", extId },
			value: {
				fontFamily: p.fontFamily,
				fontWeight: p.fontWeight,
				fontSizePct: p.fontSizePct,
				position: p.position,
				align: p.align,
				offsetPct: p.offsetPct,
				color: p.color,
				// New pill and highlight fields default from the base style, so older packs still register a complete look.
				mutedColor: p.mutedColor ?? DEFAULT_CAPTION_STYLE.mutedColor,
				uppercase: p.uppercase,
				letterSpacing: p.letterSpacing,
				background: p.background,
				backgroundColor: p.backgroundColor,
				backgroundOpacity: p.backgroundOpacity,
				boxPaddingXEm: p.boxPaddingXEm ?? DEFAULT_CAPTION_STYLE.boxPaddingXEm,
				boxPaddingYEm: p.boxPaddingYEm ?? DEFAULT_CAPTION_STYLE.boxPaddingYEm,
				boxRadiusEm: p.boxRadiusEm ?? DEFAULT_CAPTION_STYLE.boxRadiusEm,
				lineHeight: p.lineHeight ?? DEFAULT_CAPTION_STYLE.lineHeight,
				outlineWidth: p.outlineWidth,
				outlineColor: p.outlineColor,
				maxLines: p.maxLines,
				maxCharsPerLine: p.maxCharsPerLine ?? DEFAULT_CAPTION_STYLE.maxCharsPerLine,
				animation: p.animation,
			},
		});
	}

	if (entries.length > 0) registry.registerMany(entries);
	return entries.length;
}

/** Remove every entry a pack contributed (uninstall / disable). */
export function unregisterExtension(extId: string): void {
	registry.unregisterExtension(extId);
}
