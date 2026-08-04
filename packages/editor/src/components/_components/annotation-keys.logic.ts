/** Nudge step in device pixels; Shift switches to the coarse step. */
export const NUDGE_STEP_PX = 1;
export const NUDGE_STEP_COARSE_PX = 10;

export interface KeyChord {
	key: string;
	altKey: boolean;
	ctrlKey: boolean;
	metaKey: boolean;
	shiftKey: boolean;
}

/**
 * Device-pixel offset for a nudge, or null when the chord isn't one.
 *
 * Alt is required. A BARE arrow is the transport's frame-step, advertised in the
 * player tooltips, so nudging on plain arrows meant a selected shape silently
 * took over a documented global key.
 */
export function nudgeVectorPx(e: KeyChord): { dx: number; dy: number } | null {
	if (!e.altKey || e.ctrlKey || e.metaKey) return null;
	const step = e.shiftKey ? NUDGE_STEP_COARSE_PX : NUDGE_STEP_PX;
	switch (e.key) {
		case "ArrowLeft":
			return { dx: -step, dy: 0 };
		case "ArrowRight":
			return { dx: step, dy: 0 };
		case "ArrowUp":
			return { dx: 0, dy: -step };
		case "ArrowDown":
			return { dx: 0, dy: step };
		default:
			return null;
	}
}
