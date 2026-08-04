/**
 * Built-in catalog registration: the single place where Recast's shipped
 * visual presets enter the registry. Imported for its side effect by
 * `lib/registry/index.ts`, so any consumer that imports `$lib/registry` is
 * guaranteed to see built-ins registered.
 *
 * Built-in entries keep their *exact legacy storage form* as their `id`
 * (cursor → bare style id; background → `asset:<id>` wire string; gradient →
 * CSS string; colour → hex; easing/smoothing → preset id). That's what makes
 * old project files resolve unchanged, with no migration.
 *
 * Dependency direction: this module imports the catalog source files; those
 * files must NOT import the registry index back (editor-store imports the
 * narrow `resolve.ts` only), so there is no import cycle.
 */

import { CURSOR_STYLES } from "../cursor/styles";
import { SMOOTHING_PRESETS } from "../cursor/smoothing";
import { EASING_PRESETS } from "../easing/cubic-bezier";
import {
	CAPTION_PRESETS,
	COLOR_PRESETS,
	GRADIENT_PRESETS,
	WALLPAPERS,
	wallpaperBackgroundValue,
} from "../../stores/editor-store.svelte";
import { registry } from "./registry.svelte";
import type { RegistryEntry, Source } from "./types";

const BUILTIN: Source = { kind: "builtin" };

let registered = false;

/** Idempotent: safe to call more than once. */
export function registerBuiltins(): void {
	if (registered) return;
	registered = true;

	registry.registerMany(
		CURSOR_STYLES.map(
			(s): RegistryEntry<"cursor"> => ({
				id: s.id,
				kind: "cursor",
				label: s.label,
				description: s.description,
				source: BUILTIN,
				value: {
					svg: s.svg,
					pressedSvg: s.pressedSvg,
					rightPressedSvg: s.rightPressedSvg,
					dragSvg: s.dragSvg,
					hotspot: s.hotspot,
					pressedHotspot: s.pressedHotspot,
					rightPressedHotspot: s.rightPressedHotspot,
					dragHotspot: s.dragHotspot,
				},
			}),
		),
	);

	registry.registerMany(
		WALLPAPERS.map((w): RegistryEntry<"background"> => {
			const wire = wallpaperBackgroundValue(w.id);
			return {
				id: wire,
				kind: "background",
				label: w.label,
				source: BUILTIN,
				thumbAssetId: w.id,
				value: { wireValue: wire },
			};
		}),
	);

	registry.registerMany(
		GRADIENT_PRESETS.map(
			(g): RegistryEntry<"gradient"> => ({
				id: g.value,
				kind: "gradient",
				label: g.label,
				source: BUILTIN,
				value: { value: g.value },
			}),
		),
	);

	registry.registerMany(
		COLOR_PRESETS.map(
			(preset): RegistryEntry<"color"> => ({
				id: preset.value,
				kind: "color",
				label: preset.label,
				source: BUILTIN,
				value: { value: preset.value },
			}),
		),
	);

	registry.registerMany(
		EASING_PRESETS.map(
			(e): RegistryEntry<"easing"> => ({
				id: e.id,
				kind: "easing",
				label: e.label,
				source: BUILTIN,
				value: { value: e.value },
			}),
		),
	);

	registry.registerMany(
		SMOOTHING_PRESETS.map(
			(p): RegistryEntry<"smoothing"> => ({
				id: p.id,
				kind: "smoothing",
				label: p.label,
				source: BUILTIN,
				value: {
					smoothing: p.smoothing,
					snapToClicks: p.snapToClicks,
					snapWindowMs: p.snapWindowMs,
				},
			}),
		),
	);

	registry.registerMany(
		CAPTION_PRESETS.map(
			(p): RegistryEntry<"captionPreset"> => ({
				id: p.id,
				kind: "captionPreset",
				label: p.label,
				description: p.description,
				source: BUILTIN,
				value: p.style,
			}),
		),
	);
}

registerBuiltins();
