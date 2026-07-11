/**
 * Smoothed transfer-rate tracker for upload progress. Feed it cumulative
 * bytes-sent per progress event; it returns an exponential moving average of
 * bytes/sec, gated so tiny time deltas don't produce noisy spikes. One tracker
 * per store, keyed by upload id. `now` is injectable for tests.
 */
export function createRateTracker() {
	const samples = new Map<string, { bytes: number; time: number; rate: number }>();

	/**
	 * Record a cumulative byte count and return the smoothed rate (bytes/sec),
	 * or `undefined` until there is enough signal to estimate one.
	 */
	function sample(
		key: string,
		bytes: number,
		now: number = Date.now(),
	): number | undefined {
		const prev = samples.get(key);
		if (!prev) {
			samples.set(key, { bytes, time: now, rate: 0 });
			return undefined;
		}
		const dt = (now - prev.time) / 1000;
		// Too soon since the last sample: keep the last estimate, don't divide by
		// a near-zero interval.
		if (dt < 0.2) return prev.rate || undefined;
		const inst = Math.max(0, (bytes - prev.bytes) / dt);
		const rate = prev.rate ? prev.rate * 0.6 + inst * 0.4 : inst;
		samples.set(key, { bytes, time: now, rate });
		return rate || undefined;
	}

	function clear(key: string) {
		samples.delete(key);
	}

	return { sample, clear };
}
