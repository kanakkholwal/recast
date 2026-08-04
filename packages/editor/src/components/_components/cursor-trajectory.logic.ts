// Pure helpers for the cursor-trajectory sparkline: even down-sampling and the
// normalised SVG path builder. The .svelte owns smoothing derivation + markup.

/** Evenly down-sample to at most `target` points, preserving order and endpoints roughly. */
export function decimate<T>(arr: T[], target: number): T[] {
	if (arr.length <= target) return arr;
	const out: T[] = new Array(target);
	const step = arr.length / target;
	for (let i = 0; i < target; i++) {
		out[i] = arr[Math.floor(i * step)];
	}
	return out;
}

/** SVG path from sample points normalised to the [0..1, 0..1] viewBox; "" when there's nothing drawable. */
export function pathFrom(
	points: { x: number; y: number }[],
	videoWidth: number,
	videoHeight: number,
	maxPoints: number,
): string {
	if (points.length === 0 || videoWidth <= 0 || videoHeight <= 0) return "";
	const pts = decimate(points, maxPoints);
	const xs = (p: { x: number }) => p.x / videoWidth;
	const ys = (p: { y: number }) => p.y / videoHeight;
	return pts
		.map((p, i) => `${i === 0 ? "M" : "L"} ${xs(p).toFixed(4)} ${ys(p).toFixed(4)}`)
		.join(" ");
}
