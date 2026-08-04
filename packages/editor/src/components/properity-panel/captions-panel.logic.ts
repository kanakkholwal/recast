/**
 * CaptionsPanel helpers: model grouping / default selection, device + language
 * labels, download progress, and the preset-match test that drives the theme
 * picker's "active theme vs. Custom" readout.
 */

import { resolveCaptionAnimation } from "@recast/captions";
import type { CaptionModelInfo, DeviceCapabilities, TranscriptSegment } from "../../lib/wire-types";
import type { CaptionPresetValue } from "../../lib/registry/types";
import type { CaptionStyle } from "../../stores/editor-store.svelte";

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
	const usable = models.filter((m) => m.installed && m.runnable && m.runtimeAvailable);
	return (
		usable.find((m) => m.isDefault)?.id ??
		usable[0]?.id ??
		models.find((m) => m.isDefault)?.id ??
		models[0]?.id ??
		null
	);
}

/** Language badge: the covered-language count when the registry knows it
 *  ("28 languages" reads better than "Multilingual"), else "Multilingual" for
 *  `multi`, else the codes upper-cased. */
export function langLabel(m: CaptionModelInfo): string {
	if (m.languageCount && m.languageCount > 1) return `${m.languageCount} languages`;
	return m.languages.includes("multi") ? "Multilingual" : m.languages.join(", ").toUpperCase();
}

/** Compute backend label: the GPU backend when available, else "CPU only". */
export function gpuLabel(caps: DeviceCapabilities | null): string {
	if (!caps) return "";
	return caps.gpu.available ? (caps.gpu.backend?.toUpperCase() ?? "GPU") : "CPU only";
}

/** Transcript lines containing `query` (case-insensitive substring). Returns the
 *  same array reference for a blank query, so the list does not re-key. */
export function filterSegments(segments: TranscriptSegment[], query: string): TranscriptSegment[] {
	const q = query.trim().toLowerCase();
	if (!q) return segments;
	return segments.filter((s) => s.text.toLowerCase().includes(q));
}

/** Wall-clock label for a running job: `0s` under a minute, then `m:ss`. */
export function elapsedLabel(ms: number): string {
	const total = Math.max(0, Math.floor(ms / 1000));
	if (total < 60) return `${total}s`;
	return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/** Download percent (0..100), clamped; 0 when total size is unknown. */
export function downloadProgressPct(downloaded: number, total: number): number {
	return total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0;
}

/**
 * Whether the current caption style equals a preset field-for-field, INCLUDING
 * the pill fields and the (resolved) animation. This is the test behind "which
 * preset is active" (null -> the user has tweaked to Custom). Animation must be
 * compared: two presets can differ only in highlight/entrance, and a preset
 * that adds progressive highlight to an otherwise-identical look would otherwise
 * read as already active.
 */
export function captionStyleMatchesPreset(cs: CaptionStyle, v: CaptionPresetValue): boolean {
	const styleMatches =
		v.fontFamily === cs.fontFamily &&
		v.fontWeight === cs.fontWeight &&
		v.fontSizePct === cs.fontSizePct &&
		v.position === cs.position &&
		v.align === cs.align &&
		v.offsetPct === cs.offsetPct &&
		v.color === cs.color &&
		v.mutedColor === cs.mutedColor &&
		v.uppercase === cs.uppercase &&
		v.letterSpacing === cs.letterSpacing &&
		v.background === cs.background &&
		v.backgroundColor === cs.backgroundColor &&
		v.backgroundOpacity === cs.backgroundOpacity &&
		v.boxPaddingXEm === cs.boxPaddingXEm &&
		v.boxPaddingYEm === cs.boxPaddingYEm &&
		v.boxRadiusEm === cs.boxRadiusEm &&
		v.lineHeight === cs.lineHeight &&
		v.outlineWidth === cs.outlineWidth &&
		v.outlineColor === cs.outlineColor &&
		v.maxLines === cs.maxLines &&
		v.maxCharsPerLine === cs.maxCharsPerLine;
	if (!styleMatches) return false;

	const a = resolveCaptionAnimation(v.animation);
	const b = resolveCaptionAnimation(cs.animation);
	return (
		a.chunk === b.chunk &&
		a.chunkSize === b.chunkSize &&
		a.emphasis === b.emphasis &&
		a.emphasisColor === b.emphasisColor &&
		(a.highlight ?? "none") === (b.highlight ?? "none") &&
		a.entrance === b.entrance &&
		a.entranceMs === b.entranceMs &&
		a.holdGaps === b.holdGaps
	);
}
