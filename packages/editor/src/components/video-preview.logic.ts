/**
 * Parity-sensitive preview maths, extracted as pure param-in functions. Each
 * mirrors a Rust export path 1:1: preview and the rendered MP4 MUST agree, so
 * any behaviour change here is a regression. Callers thread the relevant store
 * slice in as an argument (zoom regions, cursor samples, idle periods, …).
 */

import type { Easing } from "$lib/easing/cubic-bezier";
import type { ZoomRegion } from "$lib/stores/editor-store.svelte";
import { activeZoomIndex } from "$lib/zoom/resolve";
// Runtime import via relative path (not `$lib`): the standalone vitest config
// has no `$lib` alias, and this module is unit-tested. Type-only `$lib` imports
// elsewhere are fine; they're erased before the test runs.
import { bezierY } from "../../lib/easing/cubic-bezier";

export type CursorSampleJS = {
	timestampUs: number;
	x: number;
	y: number;
	visible: boolean;
	leftDown: boolean;
	rightDown: boolean;
};

export type IdlePeriodJS = { startUs: number; endUs: number };

export interface ZoomState {
	scale: number;
	cx: number;
	cy: number;
	motionBlur: number;
}

/**
 * Zoom state for `timeSec`: eased scale (1.0 outside any region), focus
 * centre in video UV, and motion-blur strength. Matches the Rust
 * `ZoomRegion::scale_at` 1:1 so preview and export stay aligned.
 */
export function evaluateZoomAt(regions: ZoomRegion[], timeSec: number): ZoomState {
	const active = activeZoomIndex(regions, timeSec);
	if (active !== -1) {
		const r = regions[active];
		const duration = Math.max(0, r.end - r.start);
		const half = duration * 0.5;
		const rampIn = Math.min(Math.max(0, r.rampIn), half);
		const rampOut = Math.min(Math.max(0, r.rampOut), half);
		const holdStart = r.start + rampIn;
		const holdEnd = r.end - rampOut;
		let phase: number;
		let curve;
		let atHold = false;
		if (timeSec < holdStart) {
			phase = rampIn > 0 ? (timeSec - r.start) / rampIn : 1;
			curve = r.easeIn;
		} else if (timeSec > holdEnd) {
			phase = rampOut > 0 ? (r.end - timeSec) / rampOut : 1;
			curve = r.easeOut;
		} else {
			atHold = true;
			phase = 1;
			curve = r.easeIn;
		}
		phase = Math.max(0, Math.min(1, phase));
		const eased = atHold ? 1 : bezierY(curve, phase);
		const scale = 1.0 + (r.scale - 1.0) * eased;
		// Focus point is CONSTANT at the target for the whole region; only the
		// scale eases. The affine zoom `(uv - c)/scale + c` is the identity at
		// scale≈1 (no first-frame offset regardless of c) and dollies straight
		// into the target as it ramps. Easing the centre from 0.5→target instead
		// caused the "scale at centre, then slide" artifact, and a constant
		// centre keeps the cursor (same forward transform) glued.
		const cx = r.centerX ?? 0.5;
		const cy = r.centerY ?? 0.5;
		return { scale, cx, cy, motionBlur: r.motionBlur ?? 0 };
	}
	return { scale: 1.0, cx: 0.5, cy: 0.5, motionBlur: 0 };
}

/**
 * Cursor position/state at `timestampUs`, mirror of
 * `cursor::smoothing::interpolate_at`. Samples must be sorted by timestamp.
 * `easing` reshapes the interpolation parameter between adjacent captured
 * samples; boolean states still flip at the midpoint of the linear param.
 */
export function interpolateCursor(
	cursorSamples: CursorSampleJS[],
	easing: Easing | null,
	timestampUs: number,
): CursorSampleJS | null {
	if (cursorSamples.length === 0) return null;
	// Binary search
	let lo = 0;
	let hi = cursorSamples.length;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (cursorSamples[mid].timestampUs < timestampUs) lo = mid + 1;
		else hi = mid;
	}
	const idx = lo;
	if (idx >= cursorSamples.length) return cursorSamples[cursorSamples.length - 1];
	if (idx === 0 || cursorSamples[idx].timestampUs === timestampUs) return cursorSamples[idx];
	const a = cursorSamples[idx - 1];
	const b = cursorSamples[idx];
	const range = b.timestampUs - a.timestampUs;
	const tLinear = range > 0 ? (timestampUs - a.timestampUs) / range : 0;
	// Apply the user's cursor-motion easing if set. The curve reshapes
	// the *interpolation parameter* between adjacent captured samples;
	// boolean states still flip at the midpoint of the linear param to
	// keep click/release timing predictable.
	const t = easing ? bezierY(easing, tLinear) : tLinear;
	return {
		timestampUs,
		x: a.x + (b.x - a.x) * t,
		y: a.y + (b.y - a.y) * t,
		visible: tLinear < 0.5 ? a.visible : b.visible,
		leftDown: tLinear < 0.5 ? a.leftDown : b.leftDown,
		rightDown: tLinear < 0.5 ? a.rightDown : b.rightDown,
	};
}

// Idle hide fade: shared 200ms ramp at each end of an idle period.
// Mirrored 1:1 in `cursor_export.rs` so preview and export agree.
export const CURSOR_IDLE_FADE_US = 200_000;

/**
 * Cursor idle-hide alpha at `tsUs`: 1 outside any idle period, 0 deep inside,
 * with a symmetric CURSOR_IDLE_FADE_US ramp at each boundary.
 */
export function idleAlphaAt(
	idlePeriods: IdlePeriodJS[],
	tsUs: number,
	idleTimeoutSec: number,
): number {
	const thresholdUs = idleTimeoutSec * 1_000_000;
	for (const period of idlePeriods) {
		const fadeStart = period.startUs + thresholdUs;
		if (period.endUs <= fadeStart) continue;
		const fadeEnd = Math.min(fadeStart + CURSOR_IDLE_FADE_US, period.endUs);
		const resumeStart = Math.max(period.endUs - CURSOR_IDLE_FADE_US, fadeEnd);
		if (tsUs < fadeStart || tsUs > period.endUs) continue;
		if (tsUs >= fadeEnd && tsUs <= resumeStart) return 0;
		if (tsUs < fadeEnd) {
			return 1 - (tsUs - fadeStart) / (fadeEnd - fadeStart);
		}
		return 1 - (period.endUs - tsUs) / (period.endUs - resumeStart);
	}
	return 1;
}

// Coarse resolution bucket for telemetry cohorting (the default-on decision
// is "decode-fps by OS + resolution"). Keyed off the larger dimension.
export function resolutionTier(w: number, h: number): string {
	const p = Math.max(w, h);
	if (p >= 4500) return "5k";
	if (p >= 3000) return "4k";
	if (p >= 2000) return "1440p";
	if (p >= 1700) return "1080p";
	if (p >= 1200) return "720p";
	return "sd";
}

/** MediaBunny error codes that are transient (a GPU-process reset / TDR under
 *  scrub-thrash), not a property of the file — worth an automatic rebuild. */
const MB_TRANSIENT_CODES: ReadonlySet<string> = new Set([
	"internal",
	"worker-died",
	"decode-failed",
]);

/**
 * Whether a mid-stream MediaBunny decode failure should trigger an automatic
 * source rebuild rather than a permanent drop to the `<video>` element. A GPU
 * reset kills the decoder transiently, so rebuilding recovers full quality; an
 * `unsupported`/`bad-input` codec would just fail again, so we degrade instead.
 * `attempts` is the number already made in the current failure streak.
 */
export function shouldRecoverMbSource(code: string, attempts: number, maxAttempts = 3): boolean {
	return MB_TRANSIENT_CODES.has(code) && attempts < maxAttempts;
}

// Map a source-init failure to a coarse, PII-safe reason. The raw message can
// in principle carry a URL/path, so we NEVER send it; only this enum.
export function classifyMbError(err: unknown): string {
	const m = (err instanceof Error ? err.message : String(err)).toLowerCase();
	if (m.includes("unavailable") || m.includes("worker") || m.includes("videoframe"))
		return "unsupported";
	if (m.includes("track")) return "no_video_track";
	if (m.includes("codec") || m.includes("config") || m.includes("decoder"))
		return "codec_unsupported";
	if (m.includes("http") || m.includes("fetch")) return "fetch_failed";
	return "decode_error";
}
