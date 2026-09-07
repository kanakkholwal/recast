/**
 * `performance.measure` instrumentation for the decode pipeline
 * (REQUIREMENTS.md §5). Stage timings show up on the DevTools Performance
 * timeline under the `recast-media:` prefix, which is how the §3 latency rows
 * get checked by hand until a browser harness exists.
 *
 * Self-limiting: measures are cleared once the buffer passes `MAX_ENTRIES`, so
 * a long editing session can't grow the performance buffer without bound.
 */

const PREFIX = "recast-media";
const MAX_ENTRIES = 200;

let emitted = 0;

function canMeasure(): boolean {
	return (
		typeof performance !== "undefined" &&
		typeof performance.measure === "function" &&
		typeof performance.now === "function"
	);
}

/**
 * Record a completed stage as a measure spanning `startMs` → now. Never throws:
 * instrumentation must not be able to break playback.
 */
export function measureSince(stage: string, startMs: number, detail?: unknown): void {
	if (!canMeasure()) return;
	try {
		const name = `${PREFIX}:${stage}`;
		performance.measure(name, { start: startMs, end: performance.now(), detail });
		if (++emitted > MAX_ENTRIES) {
			emitted = 0;
			performance.clearMeasures?.();
		}
	} catch {
		/* measure is best-effort */
	}
}

/** Monotonic timestamp for pairing with `measureSince`, or 0 when unavailable. */
export function markNow(): number {
	return typeof performance !== "undefined" && typeof performance.now === "function"
		? performance.now()
		: 0;
}
