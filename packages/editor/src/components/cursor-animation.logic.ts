/**
 * Click/press animation maths for the cursor overlay. The captured cursor track
 * is collapsed into one `PressEvent` per click; the press/click visuals are
 * deterministic functions of time around those events. Mirrors `cursor_anim.rs`
 * so editor preview and export agree frame-for-frame.
 */

/** One collapsed click: down/up time + the captured click position. */
export type PressEvent = {
	downUs: number;
	upUs: number;
	downX: number;
	downY: number;
	/** Right button initiated this press (selects the sprite slot only). */
	right: boolean;
	/** Cursor moved past DRAG_THRESHOLD_PX (source px) while held. */
	dragged: boolean;
};

/** The subset of a captured cursor sample that press detection needs. */
export interface PressSample {
	timestampUs: number;
	x: number;
	y: number;
	leftDown: boolean;
	rightDown: boolean;
}

/** Cursor displacement (source px) during a hold beyond which it's a drag.
 *  MUST mirror DRAG_THRESHOLD_PX in cursor_anim.rs. */
const DRAG_THRESHOLD_PX = 8;

const PRESS_MIN_HOLD_US = 320_000; // minimum down-window from the click frame
const PRESS_LINGER_US = 320_000; // pressed sprite holds this long past release
const PRESS_PREROLL_US = 320_000; // pointer sprite visible this long before click
const PRESS_VIS_RAMP_US = 180_000; // cursor visibility ramp-in / ramp-out
const PRESS_POSTROLL_US = 320_000; // visibility holds this long after sprite settles
const PRESS_ANTICIP_US = 140_000; // anticipation lift duration
const PRESS_RECOVERY_US = 380_000; // recovery duration after click snap
const PRESS_LIFT = 0.04; // anticipation peak: scale = 1 + LIFT
const PRESS_PUNCH = 0.16; // click compression: scale = 1 - PUNCH
const PRESS_BOUNCE = 0.03; // recovery overshoot above 1
// Always-on click snap: a cosine ramp pulls the rendered cursor x/y through the
// captured click anchor inside ±CLICK_SNAP_HALF_US so the impact frame ALWAYS
// lands on the click target, regardless of smoothing strength / snap toggle.
const CLICK_SNAP_HALF_US = 200_000;
const HIGHLIGHT_FADE_IN_US = 40_000;
const HIGHLIGHT_FADE_OUT_US = 220_000;

/** 3t² − 2t³ smoothstep, clamped, C1-continuous, cheap. */
export function smoothStep01(t: number): number {
	if (t <= 0) return 0;
	if (t >= 1) return 1;
	return t * t * (3 - 2 * t);
}

/**
 * Collapse raw cursor samples into one press event per click. Button state is
 * read from RAW samples: smoothing reshapes x/y but must never move click timing/position.
 */
export function buildPressEvents(samples: PressSample[]): PressEvent[] {
	const events: PressEvent[] = [];
	if (samples.length === 0) return events;
	let inPress = false;
	let downUs = 0;
	let downX = 0;
	let downY = 0;
	let right = false;
	let maxDisp = 0;
	for (let i = 0; i < samples.length; i++) {
		const s = samples[i];
		const down = s.leftDown || s.rightDown;
		if (down && !inPress) {
			inPress = true;
			downUs = s.timestampUs;
			downX = s.x;
			downY = s.y;
			// Right-click only when right is the sole button down at the edge.
			right = s.rightDown && !s.leftDown;
			maxDisp = 0;
		} else if (down && inPress) {
			maxDisp = Math.max(maxDisp, Math.hypot(s.x - downX, s.y - downY));
		} else if (!down && inPress) {
			inPress = false;
			events.push({
				downUs,
				upUs: s.timestampUs,
				downX,
				downY,
				right,
				dragged: maxDisp > DRAG_THRESHOLD_PX,
			});
		}
	}
	if (inPress) {
		events.push({
			downUs,
			upUs: samples[samples.length - 1].timestampUs,
			downX,
			downY,
			right,
			dragged: maxDisp > DRAG_THRESHOLD_PX,
		});
	}
	return events;
}

type PressIndex = { downs: Float64Array; maxSpan: number };

// Keyed on array identity: `pressEvents` is rebuilt wholesale from the raw
// track, so a new array IS the invalidation signal.
const indexCache = new WeakMap<PressEvent[], PressIndex>();

function pressIndex(events: PressEvent[]): PressIndex {
	const cached = indexCache.get(events);
	if (cached) return cached;
	const downs = new Float64Array(events.length);
	let maxSpan = 0;
	for (let i = 0; i < events.length; i++) {
		const ev = events[i];
		downs[i] = ev.downUs;
		const holdEnd = Math.max(ev.upUs + PRESS_LINGER_US, ev.downUs + PRESS_MIN_HOLD_US);
		// Widest of the three windows (pressStateAt's visEnd), so one index serves all.
		const span = holdEnd + PRESS_POSTROLL_US + PRESS_VIS_RAMP_US - ev.downUs;
		if (span > maxSpan) maxSpan = span;
	}
	const index = { downs, maxSpan };
	indexCache.set(events, index);
	return index;
}

/**
 * First event whose influence window can still reach `tsUs`. A held press makes
 * window *end* non-monotonic, so the bound is `downUs >= tsUs - maxSpan` rather
 * than a search on the end time.
 */
function firstCandidate(events: PressEvent[], tsUs: number): number {
	const { downs, maxSpan } = pressIndex(events);
	const cutoff = tsUs - maxSpan;
	let lo = 0;
	let hi = downs.length;
	while (lo < hi) {
		const mid = (lo + hi) >> 1;
		if (downs[mid] < cutoff) lo = mid + 1;
		else hi = mid;
	}
	return lo;
}

/**
 * Active click anchor + cosine falloff weight at `tsUs`, or null outside any
 * click-snap window. Picks the closest event when two windows overlap so
 * double-clicks snap cleanly to each successive target.
 */
export function clickAnchorAt(
	events: PressEvent[],
	tsUs: number,
): { x: number; y: number; weight: number } | null {
	let bestEv: PressEvent | null = null;
	let bestAbsDt = Infinity;
	for (let i = firstCandidate(events, tsUs); i < events.length; i++) {
		const ev = events[i];
		if (tsUs < ev.downUs - CLICK_SNAP_HALF_US) break;
		if (tsUs > ev.downUs + CLICK_SNAP_HALF_US) continue;
		const absDt = Math.abs(tsUs - ev.downUs);
		if (absDt < bestAbsDt) {
			bestAbsDt = absDt;
			bestEv = ev;
		}
	}
	if (!bestEv) return null;
	// Cosine ramp: 1 at the click frame, 0 at the window edge.
	const weight = 0.5 + 0.5 * Math.cos((bestAbsDt / CLICK_SNAP_HALF_US) * Math.PI);
	return { x: bestEv.downX, y: bestEv.downY, weight };
}

/**
 * Pinned click-highlight envelope: the CAPTURED click position and an alpha
 * that rises the instant the click lands, holds through the press, then fades.
 */
export function clickHighlightAt(
	events: PressEvent[],
	tsUs: number,
): { x: number; y: number; alpha: number } | null {
	let best: PressEvent | null = null;
	let bestDt = Infinity;
	for (let i = firstCandidate(events, tsUs); i < events.length; i++) {
		const ev = events[i];
		const holdEnd = Math.max(ev.upUs + PRESS_LINGER_US, ev.downUs + PRESS_MIN_HOLD_US);
		if (tsUs < ev.downUs) break;
		if (tsUs > holdEnd + HIGHLIGHT_FADE_OUT_US) continue;
		const dt = tsUs - ev.downUs;
		if (dt < bestDt) {
			bestDt = dt;
			best = ev;
		}
	}
	if (!best) return null;
	const holdEnd = Math.max(best.upUs + PRESS_LINGER_US, best.downUs + PRESS_MIN_HOLD_US);
	let alpha: number;
	if (tsUs < best.downUs + HIGHLIGHT_FADE_IN_US) {
		alpha = smoothStep01((tsUs - best.downUs) / HIGHLIGHT_FADE_IN_US);
	} else if (tsUs <= holdEnd) {
		alpha = 1;
	} else {
		alpha = smoothStep01((holdEnd + HIGHLIGHT_FADE_OUT_US - tsUs) / HIGHLIGHT_FADE_OUT_US);
	}
	return { x: best.downX, y: best.downY, alpha };
}

/** All click-relative sprite/scale/visibility state for a moment in time. */
export interface PressState {
	pressedSprite: boolean; // swap to the press SVG sprite
	visibleAlpha: number; // 0..1 boost on top of idleAlpha (overrides idle-hide)
	scale: number; // applied directly to the sprite transform
	right: boolean; // active press was a right-click (sprite slot)
	dragging: boolean; // active press is a drag (sprite slot)
}

/**
 * Press animation state at `tsUs`. When two events' influence windows overlap
 * (sub-300ms double-clicks) the one whose `downUs` is closest to `tsUs` wins, so
 * the sprite, anticipation lift, and impact snap all track the click the viewer
 * is about to perceive.
 */
export function pressStateAt(events: PressEvent[], tsUs: number): PressState {
	let bestEv: PressEvent | null = null;
	let bestHoldEnd = 0;
	let bestVisStart = 0;
	let bestVisEnd = 0;
	let bestAbsDt = Infinity;
	for (let i = firstCandidate(events, tsUs); i < events.length; i++) {
		const ev = events[i];
		const holdEnd = Math.max(ev.upUs + PRESS_LINGER_US, ev.downUs + PRESS_MIN_HOLD_US);
		const visStart = ev.downUs - PRESS_PREROLL_US - PRESS_VIS_RAMP_US;
		const visEnd = holdEnd + PRESS_POSTROLL_US + PRESS_VIS_RAMP_US;
		// Events are sorted by downUs ascending, so once we're before an event's
		// window we're before every later event's window too.
		if (tsUs < visStart) break;
		if (tsUs > visEnd) continue;
		const absDt = Math.abs(tsUs - ev.downUs);
		if (absDt < bestAbsDt) {
			bestEv = ev;
			bestHoldEnd = holdEnd;
			bestVisStart = visStart;
			bestVisEnd = visEnd;
			bestAbsDt = absDt;
		}
	}
	if (!bestEv)
		return {
			pressedSprite: false,
			visibleAlpha: 0,
			scale: 1,
			right: false,
			dragging: false,
		};

	// Visibility: smooth ramp in, hold full, smooth ramp out.
	let visibleAlpha = 1;
	if (tsUs < bestEv.downUs - PRESS_PREROLL_US) {
		visibleAlpha = smoothStep01((tsUs - bestVisStart) / PRESS_VIS_RAMP_US);
	} else if (tsUs > bestHoldEnd + PRESS_POSTROLL_US) {
		visibleAlpha = smoothStep01((bestVisEnd - tsUs) / PRESS_VIS_RAMP_US);
	}

	// Pressed sprite: from preroll start through the held window.
	const pressedSprite = tsUs >= bestEv.downUs - PRESS_PREROLL_US && tsUs <= bestHoldEnd;

	// Scale curve: three phases keyed on `dt = tsUs - downUs`.
	//   dt ∈ [-ANTICIP, 0):  1 → 1+LIFT (smooth lift)
	//   dt = 0:              snap to 1-PUNCH (click frame, the sync point)
	//   dt ∈ [0, RECOVERY]:  1-PUNCH → 1+BOUNCE → 1
	let scale = 1;
	const dt = tsUs - bestEv.downUs;
	if (dt >= -PRESS_ANTICIP_US && dt < 0) {
		const u = (dt + PRESS_ANTICIP_US) / PRESS_ANTICIP_US;
		scale = 1 + PRESS_LIFT * smoothStep01(u);
	} else if (dt >= 0 && dt < PRESS_RECOVERY_US) {
		const u = dt / PRESS_RECOVERY_US;
		if (u < 0.6) {
			// 1-PUNCH → 1+BOUNCE
			const v = u / 0.6;
			scale = 1 - PRESS_PUNCH + (PRESS_PUNCH + PRESS_BOUNCE) * smoothStep01(v);
		} else {
			// 1+BOUNCE → 1
			const v = (u - 0.6) / 0.4;
			scale = 1 + PRESS_BOUNCE - PRESS_BOUNCE * smoothStep01(v);
		}
	}

	return {
		pressedSprite,
		visibleAlpha,
		scale,
		right: bestEv.right,
		dragging: bestEv.dragged,
	};
}
