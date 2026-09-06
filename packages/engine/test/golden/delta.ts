/**
 * The frame comparison the wasm golden arm gates on.
 *
 * A port of `recast_testkit::compare::frame_delta`, and its own file so it can
 * be unit-tested: a comparison that quietly returned zero would make the whole
 * arm pass without looking at anything.
 */

export interface Delta {
	maxChannel: number;
	meanChannel: number;
	differingPixels: number;
	totalPixels: number;
}

/** Channel-wise over RGBA. Alpha is included: a compositor bug that only shows
 *  in alpha is still a bug. */
export function frameDelta(a: Uint8Array, b: Uint8Array): Delta {
	if (a.length !== b.length || a.length === 0 || a.length % 4 !== 0) {
		throw new Error(`cannot compare ${a.length} bytes with ${b.length}`);
	}
	let maxChannel = 0;
	let total = 0;
	let differingPixels = 0;
	for (let i = 0; i < a.length; i += 4) {
		let differs = false;
		for (let c = 0; c < 4; c++) {
			const d = Math.abs(a[i + c] - b[i + c]);
			if (d > 0) differs = true;
			if (d > maxChannel) maxChannel = d;
			total += d;
		}
		if (differs) differingPixels++;
	}
	return {
		maxChannel,
		meanChannel: total / a.length,
		differingPixels,
		totalPixels: a.length / 4,
	};
}

export function isWithin(delta: Delta, maxChannel: number, maxMean: number): boolean {
	return delta.maxChannel <= maxChannel && delta.meanChannel <= maxMean;
}
