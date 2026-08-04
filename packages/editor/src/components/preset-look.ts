import type { BackgroundType, LayoutMode, OutputAspect } from "$lib/stores/editor-store.svelte";

/** The slice of editor state a preset owns. Everything else is left alone. */
export interface PresetLook {
	bg: BackgroundType;
	value: string;
	padding: number;
	blur: number;
	layout: LayoutMode;
	aspect: OutputAspect;
	presetId: string | null;
}

export interface PresetSource {
	id: string;
	bg: BackgroundType;
	value?: string;
	padding: number;
	blur: number;
	layout?: LayoutMode;
	aspect: string;
}

const PRESET_ASPECTS: Record<string, OutputAspect> = {
	"16:9": "16:9",
	"9:16": "9:16",
	"1:1": "1:1",
	"1.91:1": "1.91:1",
};

/** The look a preset commits. Unknown aspects (e.g. "Source") stay source-matched. */
export function commitLook(preset: PresetSource, current: PresetLook): PresetLook {
	return {
		bg: preset.bg,
		value: preset.value ?? current.value,
		padding: preset.padding,
		blur: preset.blur,
		layout: preset.layout ?? current.layout,
		aspect: PRESET_ASPECTS[preset.aspect] ?? "source",
		// UI-only: lets the toolbar surface the applied preset as a chip.
		presetId: preset.id,
	};
}

/**
 * The look a preset previews: identical, except `presetId` stays put. That field
 * feeds the picker's `currentId`, which regroups the list and moves the cursor,
 * which re-fires the preview — an infinite effect loop. "Applied" means committed.
 */
export function previewLook(preset: PresetSource, current: PresetLook): PresetLook {
	return { ...commitLook(preset, current), presetId: current.presetId };
}
