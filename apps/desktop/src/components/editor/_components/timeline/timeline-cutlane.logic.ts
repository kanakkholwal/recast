// Clamp maths for dragging cut bands, on the original-recording axis.

export interface CutBounds {
	start: number;
	end: number;
}

// Translate a band by delta, pushing it back inside [0, duration] without
// changing its length when it would run past either edge.
export function clampCutMove(p: {
	originStart: number;
	originEnd: number;
	delta: number;
	duration: number;
}): CutBounds {
	let s = p.originStart + p.delta;
	let en = p.originEnd + p.delta;
	if (s < 0) {
		en -= s;
		s = 0;
	}
	if (en > p.duration) {
		s -= en - p.duration;
		en = p.duration;
	}
	return { start: Math.max(0, s), end: en };
}

// Move one edge, holding the other and never crossing (min gap = minCut).
export function clampCutResize(p: {
	edge: "l" | "r";
	originStart: number;
	originEnd: number;
	delta: number;
	duration: number;
	minCut: number;
}): CutBounds {
	if (p.edge === "l") {
		const s = Math.min(
			Math.max(0, p.originStart + p.delta),
			p.originEnd - p.minCut,
		);
		return { start: s, end: p.originEnd };
	}
	const en = Math.max(
		Math.min(p.duration, p.originEnd + p.delta),
		p.originStart + p.minCut,
	);
	return { start: p.originStart, end: en };
}
