// Unit-interval cubic-bezier evaluator. Given the two middle control points
// (x1, y1) and (x2, y2) with implicit anchors P0=(0,0) and P3=(1,1), returns
// y for a given x. Same algorithm as Blink / WebKit: solve x(t)=x via
// Newton-Raphson with a bisection fallback, then evaluate y(t).
//
// y may exceed [0,1] for overshoot curves (e.g. Bounce) — that's intentional.

export interface Easing {
	x1: number;
	y1: number;
	x2: number;
	y2: number;
}

export const LINEAR: Easing = { x1: 0, y1: 0, x2: 1, y2: 1 };
export const EASE: Easing = { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1.0 };
export const EASE_IN: Easing = { x1: 0.42, y1: 0, x2: 1.0, y2: 1.0 };
export const EASE_OUT: Easing = { x1: 0, y1: 0, x2: 0.58, y2: 1.0 };
export const EASE_IN_OUT: Easing = { x1: 0.42, y1: 0, x2: 0.58, y2: 1.0 };
export const SNAP: Easing = { x1: 0, y1: 0, x2: 0.1, y2: 1.0 };
export const BOUNCE: Easing = { x1: 0.68, y1: -0.55, x2: 0.27, y2: 1.55 };

export const EASING_PRESETS: { id: string; label: string; value: Easing }[] = [
	{ id: "linear", label: "Linear", value: LINEAR },
	{ id: "ease", label: "Ease", value: EASE },
	{ id: "ease-in", label: "Ease In", value: EASE_IN },
	{ id: "ease-out", label: "Ease Out", value: EASE_OUT },
	{ id: "ease-in-out", label: "In-Out", value: EASE_IN_OUT },
	{ id: "snap", label: "Snap", value: SNAP },
	{ id: "bounce", label: "Bounce", value: BOUNCE },
];

// Polynomial form coefficients (see https://pomax.github.io/bezierinfo/).
function coeffs(c1: number, c2: number): [number, number, number] {
	const a = 1 - 3 * c2 + 3 * c1;
	const b = 3 * c2 - 6 * c1;
	const c = 3 * c1;
	return [a, b, c];
}

function sample(t: number, a: number, b: number, c: number): number {
	return ((a * t + b) * t + c) * t;
}

function sampleDerivative(t: number, a: number, b: number, c: number): number {
	return (3 * a * t + 2 * b) * t + c;
}

export function bezierY(easing: Easing, x: number): number {
	const { x1, y1, x2, y2 } = easing;
	// Degenerate / identity linear — skip the solve.
	if (x1 === y1 && x2 === y2) return x;
	if (x <= 0) return 0;
	if (x >= 1) return 1;

	const [ax, bx, cx] = coeffs(x1, x2);
	const [ay, by, cy] = coeffs(y1, y2);

	// Newton-Raphson, up to 8 iterations, 1e-6 tolerance.
	let t = x;
	for (let i = 0; i < 8; i++) {
		const xt = sample(t, ax, bx, cx) - x;
		if (Math.abs(xt) < 1e-6) return sample(t, ay, by, cy);
		const dxt = sampleDerivative(t, ax, bx, cx);
		if (Math.abs(dxt) < 1e-6) break;
		t -= xt / dxt;
	}

	// Bisection fallback — guaranteed convergence on a monotonic x(t).
	let lo = 0;
	let hi = 1;
	t = x;
	while (lo < hi) {
		const xt = sample(t, ax, bx, cx);
		if (Math.abs(xt - x) < 1e-6) return sample(t, ay, by, cy);
		if (x > xt) lo = t;
		else hi = t;
		t = (lo + hi) / 2;
		if (hi - lo < 1e-7) break;
	}
	return sample(t, ay, by, cy);
}

// Build a sampled curve for drawing — returns N+1 (x,y) points from t=0..1.
// Uses the t parameter directly (not x-inverse) since we're stroking the path.
export function sampleCurve(easing: Easing, segments = 32): Array<[number, number]> {
	const { x1, y1, x2, y2 } = easing;
	const [ax, bx, cx] = coeffs(x1, x2);
	const [ay, by, cy] = coeffs(y1, y2);
	const out: Array<[number, number]> = [];
	for (let i = 0; i <= segments; i++) {
		const t = i / segments;
		out.push([sample(t, ax, bx, cx), sample(t, ay, by, cy)]);
	}
	return out;
}

export function easingEquals(a: Easing, b: Easing): boolean {
	return a.x1 === b.x1 && a.y1 === b.y1 && a.x2 === b.x2 && a.y2 === b.y2;
}
