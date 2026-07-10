/**
 * CaptionsPanel helpers: model grouping / default selection, device + language
 * labels, download progress, and the preset-match test that drives the theme
 * picker's "active theme vs. Custom" readout.
 */

import type { CaptionModelInfo, DeviceCapabilities } from "$lib/ipc";
import type { CaptionPresetValue } from "$lib/registry/types";
import type { CaptionStyle } from "$lib/stores/editor-store.svelte";

/** Models grouped by family, preserving first-seen order, for the picker. */
export function groupModelsByFamily(
	models: CaptionModelInfo[],
): { name: string; models: CaptionModelInfo[] }[] {
	const groups: { name: string; models: CaptionModelInfo[] }[] = [];
	for (const m of models) {
		let g = groups.find((x) => x.name === m.family);
		if (!g) {
			g = { name: m.family, models: [] };
			groups.push(g);
		}
		g.models.push(m);
	}
	return groups;
}

/**
 * Default model id when none is selected (or the selection went stale): prefer a
 * runnable+installed default, then any usable model, then any flagged default,
 * then the first model. Null only when the list is empty.
 */
export function pickDefaultModelId(models: CaptionModelInfo[]): string | null {
	const usable = models.filter((m) => m.installed && m.runnable);
	return (
		usable.find((m) => m.isDefault)?.id ??
		usable[0]?.id ??
		models.find((m) => m.isDefault)?.id ??
		models[0]?.id ??
		null
	);
}

/** Language badge: "Multilingual" for `multi`, else the codes upper-cased. */
export function langLabel(m: CaptionModelInfo): string {
	return m.languages.includes("multi")
		? "Multilingual"
		: m.languages.join(", ").toUpperCase();
}

/** Compute backend label: the GPU backend when available, else "CPU only". */
export function gpuLabel(caps: DeviceCapabilities | null): string {
	if (!caps) return "";
	return caps.gpu.available ? (caps.gpu.backend?.toUpperCase() ?? "GPU") : "CPU only";
}

/** Download percent (0..100), clamped; 0 when total size is unknown. */
export function downloadProgressPct(downloaded: number, total: number): number {
	return total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0;
}

/**
 * Whether the current caption style equals a preset field-for-field. This is the test
 * behind "which preset is active" (null → the user has tweaked to Custom).
 * Animation is intentionally excluded (matches the picker's original compare).
 */
export function captionStyleMatchesPreset(
	cs: CaptionStyle,
	v: CaptionPresetValue,
): boolean {
	return (
		v.fontFamily === cs.fontFamily &&
		v.fontWeight === cs.fontWeight &&
		v.fontSizePct === cs.fontSizePct &&
		v.position === cs.position &&
		v.align === cs.align &&
		v.offsetPct === cs.offsetPct &&
		v.color === cs.color &&
		v.uppercase === cs.uppercase &&
		v.letterSpacing === cs.letterSpacing &&
		v.background === cs.background &&
		v.backgroundColor === cs.backgroundColor &&
		v.backgroundOpacity === cs.backgroundOpacity &&
		v.outlineWidth === cs.outlineWidth &&
		v.outlineColor === cs.outlineColor &&
		v.maxLines === cs.maxLines
	);
}
