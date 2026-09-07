// Gaussian smoothing kills tremor, while click anchoring keeps press targets pixel-perfect through a rounded corner.

export interface CursorSampleLike {
	timestampUs: number;
	x: number;
	y: number;
	visible: boolean;
	leftDown: boolean;
	rightDown: boolean;
}

export interface ClickAnchor {
	timestampUs: number;
	x: number;
	y: number;
}

export interface SmoothingOptions {
	/** Gaussian σ in ms. `<= 0` disables smoothing (returns input). */
	sigmaMs: number;
	/** Anchor the smoothed path exactly at click x/y within `snapWindowMs` of mouse-down. */
	snapToClicks: boolean;
	/** Half-width (ms) of the snap ramp (cosine-shaped falloff). */
	snapWindowMs: number;
}

export interface SmoothResult {
	samples: CursorSampleLike[];
	clickAnchors: ClickAnchor[];
}

/** Map the UI strength slider (0..100) to a Gaussian σ in ms (0 disables). */
export function smoothingStrengthToSigmaMs(strength: number): number {
	return Math.max(0, Math.min(100, strength)) * 1.5;
}

/**
 * Smooth a cursor trajectory with a time-weighted Gaussian window, then
 * optionally pull the curve exactly through click positions inside a
 * cosine-shaped anchor window. Preserves timestamps and boolean fields.
 */
export function smoothCursorPath(raw: CursorSampleLike[], opts: SmoothingOptions): SmoothResult {
	// Detect click-down transitions even with smoothing off; callers still use these for the visualisation.
	const clickAnchors: ClickAnchor[] = [];
	for (let i = 1; i < raw.length; i++) {
		const prev = raw[i - 1];
		const curr = raw[i];
		const leftEdge = !prev.leftDown && curr.leftDown;
		const rightEdge = !prev.rightDown && curr.rightDown;
		if (leftEdge || rightEdge) {
			clickAnchors.push({ timestampUs: curr.timestampUs, x: curr.x, y: curr.y });
		}
	}

	if (raw.length < 2 || opts.sigmaMs <= 0) {
		return { samples: raw, clickAnchors };
	}

	const sigmaUs = opts.sigmaMs * 1000;
	const windowUs = sigmaUs * 3; // ±3σ catches ~99.7% of the weight
	const snapUs = Math.max(0, opts.snapWindowMs) * 1000;

	// lo and hi advance monotonically because samples are time-sorted: O(N*w) for w samples inside 3 sigma.
	const smoothed: CursorSampleLike[] = new Array(raw.length);
	let lo = 0;
	let hi = 0;
	const inv2Sigma2 = 1 / (2 * sigmaUs * sigmaUs);
	for (let i = 0; i < raw.length; i++) {
		const center = raw[i];
		const minT = center.timestampUs - windowUs;
		const maxT = center.timestampUs + windowUs;
		while (lo < raw.length && raw[lo].timestampUs < minT) lo++;
		while (hi < raw.length && raw[hi].timestampUs <= maxT) hi++;

		let sumW = 0;
		let sumX = 0;
		let sumY = 0;
		for (let j = lo; j < hi; j++) {
			const dt = raw[j].timestampUs - center.timestampUs;
			const w = Math.exp(-(dt * dt) * inv2Sigma2);
			sumW += w;
			sumX += w * raw[j].x;
			sumY += w * raw[j].y;
		}
		if (sumW > 0) {
			smoothed[i] = {
				timestampUs: center.timestampUs,
				x: sumX / sumW,
				y: sumY / sumW,
				visible: center.visible,
				leftDown: center.leftDown,
				rightDown: center.rightDown,
			};
		} else {
			smoothed[i] = center;
		}
	}

	// Cosine snap ramp, 1 at the click timestamp and 0 at the window edge, so the path glides through without a seam.
	if (opts.snapToClicks && snapUs > 0 && clickAnchors.length > 0) {
		for (const anchor of clickAnchors) {
			for (let i = 0; i < smoothed.length; i++) {
				const dt = Math.abs(smoothed[i].timestampUs - anchor.timestampUs);
				if (dt > snapUs) continue;
				const falloff = 0.5 + 0.5 * Math.cos((dt / snapUs) * Math.PI);
				const s = smoothed[i];
				smoothed[i] = {
					timestampUs: s.timestampUs,
					x: s.x * (1 - falloff) + anchor.x * falloff,
					y: s.y * (1 - falloff) + anchor.y * falloff,
					visible: s.visible,
					leftDown: s.leftDown,
					rightDown: s.rightDown,
				};
			}
		}
	}

	return { samples: smoothed, clickAnchors };
}

/** Built-in presets for the UI row. `sigmaMs` is resolved from the slider. */
export const SMOOTHING_PRESETS: Array<{
	id: string;
	label: string;
	smoothing: number;
	snapToClicks: boolean;
	snapWindowMs: number;
}> = [
	{ id: "none", label: "None", smoothing: 0, snapToClicks: false, snapWindowMs: 0 },
	{ id: "subtle", label: "Subtle", smoothing: 25, snapToClicks: true, snapWindowMs: 60 },
	{ id: "smooth", label: "Smooth", smoothing: 50, snapToClicks: true, snapWindowMs: 80 },
	{ id: "cinematic", label: "Cinematic", smoothing: 80, snapToClicks: true, snapWindowMs: 120 },
];
