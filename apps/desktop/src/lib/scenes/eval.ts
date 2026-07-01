/**
 * Pure evaluator for per-segment scene animations. Given the current time and a
 * segment's window, returns the video-layer transform (opacity, position, scale).
 * Mirrors what the WebGL preview applies to `u_videoOrigin`/`u_videoSize`/
 * `u_videoOpacity` and what the Rust export graph reproduces on the video layer —
 * the same "two evaluators stay in sync" contract as zoom, locked by the
 * scene-parity fixture.
 *
 * Evaluated in ORIGINAL/timeline time against each segment's ORIGINAL window,
 * exactly like zoom regions: the export samples the identical curve on the
 * continuous post-trim axis and the tail cut+speed stage re-times it. Animation
 * durations are therefore in source time — a sped-up clip plays its animation
 * proportionally faster, the same way its zoom ramps do.
 */

import { bezierY } from "../easing/cubic-bezier";
import type { Segment } from "../timeline/segments";
import {
	segmentAnimAt,
	clampAnimMs,
	DEFAULT_SLIDE,
	DEFAULT_SCALE_DELTA,
	DEFAULT_POP_DELTA,
	DEFAULT_ROTATE_DEG,
} from "./segment-anim";
import type { SceneAnimSpec, SegmentAnim } from "./segment-anim";

const EPS = 1e-4;

/** Video-layer transform. `translateX/Y` are fractions of the canvas width/
 *  height; `scale` multiplies the video rect about its own centre; `rotate` is
 *  degrees about that centre. */
export interface SceneTransform {
	opacity: number;
	translateX: number;
	translateY: number;
	scale: number;
	rotate: number;
}

export const SCENE_IDENTITY: SceneTransform = {
	opacity: 1,
	translateX: 0,
	translateY: 0,
	scale: 1,
	rotate: 0,
};

function clamp01(v: number): number {
	return v < 0 ? 0 : v > 1 ? 1 : v;
}

/**
 * The transform for a spec at "presence" `p`, where p=1 is the resting state
 * (fully in view, identity) and p=0 is fully animated-away. `p` may overshoot
 * [0,1] for bouncy easings, which is what gives `pop` its spring.
 */
export function presenceTransform(spec: SceneAnimSpec, p: number): SceneTransform {
	const t: SceneTransform = { ...SCENE_IDENTITY };
	switch (spec.kind) {
		case "fade":
			t.opacity = clamp01(p);
			break;
		case "slide": {
			const d = spec.intensity ?? DEFAULT_SLIDE;
			const off = (1 - p) * d;
			switch (spec.dir ?? "left") {
				case "left":
					t.translateX = -off;
					break;
				case "right":
					t.translateX = off;
					break;
				case "up":
					t.translateY = -off;
					break;
				case "down":
					t.translateY = off;
					break;
			}
			break;
		}
		case "scale":
		case "pop": {
			const amt = spec.intensity ?? (spec.kind === "pop" ? DEFAULT_POP_DELTA : DEFAULT_SCALE_DELTA);
			const startScale = 1 - amt;
			t.scale = startScale + (1 - startScale) * p;
			break;
		}
		case "shrink": {
			const amt = spec.intensity ?? DEFAULT_SCALE_DELTA;
			const startScale = 1 + amt;
			t.scale = startScale + (1 - startScale) * p;
			break;
		}
		case "rotate": {
			const deg = spec.intensity ?? DEFAULT_ROTATE_DEG;
			t.rotate = (1 - p) * deg;
			break;
		}
	}
	return t;
}

/**
 * The video-layer transform for a single segment at output time `t`, given its
 * output window `[outStart, outEnd]`. Entrance eases presence 0→1 over the first
 * `in.durationMs`; exit eases 1→0 over the last `out.durationMs`; the hold
 * between is identity. In wins if the two ramps would overlap on a short clip.
 */
export function evalSegmentTransform(
	anim: SegmentAnim | null,
	t: number,
	outStart: number,
	outEnd: number,
): SceneTransform {
	if (!anim) return SCENE_IDENTITY;
	const winDur = Math.max(0, outEnd - outStart);

	if (anim.in) {
		const d = Math.min(clampAnimMs(anim.in.durationMs) / 1000, winDur);
		if (d > 0 && t < outStart + d) {
			const phase = clamp01((t - outStart) / d);
			return presenceTransform(anim.in, bezierY(anim.in.easing, phase));
		}
	}
	if (anim.out) {
		const d = Math.min(clampAnimMs(anim.out.durationMs) / 1000, winDur);
		if (d > 0 && t > outEnd - d) {
			// phase 1 at the start of the exit window (resting), 0 at the very end.
			const phase = clamp01((outEnd - t) / d);
			return presenceTransform(anim.out, bezierY(anim.out.easing, phase));
		}
	}
	return SCENE_IDENTITY;
}

/**
 * The active scene transform at original time `t`: find the segment whose
 * original window contains `t`, then evaluate its animation. Identity when no
 * segment covers `t` or the segment has no animation. On the final frame the
 * last segment owns the boundary (NLE convention, matching `segmentAt`).
 */
export function evalSceneAt(
	segments: ReadonlyArray<Segment>,
	anims: ReadonlyArray<SegmentAnim>,
	t: number,
): SceneTransform {
	if (anims.length === 0) return SCENE_IDENTITY;
	for (const s of segments) {
		if (t >= s.start - EPS && t < s.end - EPS) {
			return evalSegmentTransform(segmentAnimAt(anims, s.start), t, s.start, s.end);
		}
	}
	const last = segments[segments.length - 1];
	if (last && Math.abs(t - last.end) <= EPS) {
		return evalSegmentTransform(segmentAnimAt(anims, last.start), t, last.start, last.end);
	}
	return SCENE_IDENTITY;
}
