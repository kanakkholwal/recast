export interface WheelGesture {
	deltaX: number;
	deltaY: number;
	shiftKey: boolean;
	ctrlKey: boolean;
	metaKey: boolean;
}

export type WheelIntent =
	| { kind: "zoom"; direction: 1 | -1 }
	| { kind: "horizontal"; delta: number }
	| { kind: "vertical"; delta: number }
	| { kind: "none" };

/** Routes a wheel gesture over the track area. `canScrollVertically` = lanes overflow the panel. */
export function wheelIntent(event: WheelGesture, canScrollVertically: boolean): WheelIntent {
	const { deltaX, deltaY } = event;

	if (event.ctrlKey || event.metaKey) {
		if (deltaY === 0) return { kind: "none" };
		return { kind: "zoom", direction: deltaY < 0 ? 1 : -1 };
	}

	// Shift+wheel is the explicit pan gesture; browsers report it as deltaX on
	// some platforms and deltaY on others, so take whichever moved.
	if (event.shiftKey) {
		const delta = deltaY !== 0 ? deltaY : deltaX;
		return delta === 0 ? { kind: "none" } : { kind: "horizontal", delta };
	}

	if (Math.abs(deltaX) > Math.abs(deltaY)) {
		return deltaX === 0 ? { kind: "none" } : { kind: "horizontal", delta: deltaX };
	}

	if (deltaY === 0) return { kind: "none" };
	// With nothing below the fold a vertical notch would do nothing, so keep
	// panning the timeline the way it did before lanes could stack.
	return canScrollVertically
		? { kind: "vertical", delta: deltaY }
		: { kind: "horizontal", delta: deltaY };
}
