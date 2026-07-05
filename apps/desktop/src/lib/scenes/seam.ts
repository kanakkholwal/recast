/**
 * Cut-seam transitions — smoothing the jump where a cut (silence or manual)
 * removed content between two kept segments. A seam transition is not a new data
 * type: it's a matched pair of per-segment animations — an exit on the segment
 * before the seam and a complementary entrance on the segment after — so it rides
 * the existing scene pipeline (../scenes/eval.ts + the Rust export graph) with no
 * new export surface. The "push" family slides the old content off while the new
 * content slides in from the opposite edge. (Dip-to-background, a paired fade,
 * lands once fade is expressible in export.)
 */

import {
	defaultSpec,
	type MotionTone,
	type SceneAnimDir,
	type SceneAnimSpec,
	type SegmentAnim,
	segmentAnimAt,
	setSegmentAnim,
} from "./segment-anim";

export type SeamTransition = "none" | "push-left" | "push-right" | "push-up" | "push-down";

/** Non-`none` push kinds, for iterating presets in the UI. */
export const PUSH_TRANSITIONS: Exclude<SeamTransition, "none">[] = [
	"push-left",
	"push-right",
	"push-up",
	"push-down",
];

// The paired directions a push writes: the left segment's exit travels `out`, the
// right segment's entrance comes FROM `in` (the opposite edge) — together reading
// as one continuous push in the exit's direction.
const PUSH_DIRS: Record<Exclude<SeamTransition, "none">, { out: SceneAnimDir; in: SceneAnimDir }> = {
	"push-left": { out: "left", in: "right" },
	"push-right": { out: "right", in: "left" },
	"push-up": { out: "up", in: "down" },
	"push-down": { out: "down", in: "up" },
};

function slideSpec(side: "in" | "out", dir: SceneAnimDir, tone: MotionTone): SceneAnimSpec {
	return { ...defaultSpec("slide", side, tone), dir };
}

/**
 * Set (or clear, for `none`) the transition across the seam between the segment
 * anchored at `leftStart` and the one at `rightStart`. Returns a new anim list —
 * the caller prunes/undo-tracks. `tone` styles the paired slides.
 */
export function setSeamTransition(
	anims: ReadonlyArray<SegmentAnim>,
	leftStart: number,
	rightStart: number,
	kind: SeamTransition,
	tone: MotionTone,
): SegmentAnim[] {
	if (kind === "none") {
		return setSegmentAnim(setSegmentAnim(anims, leftStart, "out", null), rightStart, "in", null);
	}
	const dirs = PUSH_DIRS[kind];
	return setSegmentAnim(
		setSegmentAnim(anims, leftStart, "out", slideSpec("out", dirs.out, tone)),
		rightStart,
		"in",
		slideSpec("in", dirs.in, tone),
	);
}

/**
 * The transition currently spanning a seam, read back from the two segments'
 * anims. `none` when neither side animates; `custom` when they don't form a
 * recognised push pair (e.g. the sides were tuned individually).
 */
export function seamTransitionAt(
	anims: ReadonlyArray<SegmentAnim>,
	leftStart: number,
	rightStart: number,
): SeamTransition | "custom" {
	const left = segmentAnimAt(anims, leftStart)?.out;
	const right = segmentAnimAt(anims, rightStart)?.in;
	if (!left && !right) return "none";
	if (left?.kind === "slide" && right?.kind === "slide") {
		for (const kind of PUSH_TRANSITIONS) {
			const d = PUSH_DIRS[kind];
			if (left.dir === d.out && right.dir === d.in) return kind;
		}
	}
	return "custom";
}
