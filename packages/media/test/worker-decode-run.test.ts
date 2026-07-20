import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Drives the REAL decode worker against a mocked MediaBunny. The source-side
 * harness could never catch this class of bug: it stubs the worker, so
 * anything about how the worker uses the sink was invisible to it.
 *
 * The mock enforces MediaBunny's actual canvas contract — a pooled canvas is
 * recycled round-robin, and drawing into one we transferred away (which
 * detaches it) throws.
 */

/** Stand-in for an OffscreenCanvas that a `postMessage` transfer detaches. */
class FakeCanvas {
	detached = false;
	constructor(
		readonly id: number,
		readonly width = 1920,
		readonly height = 1080,
	) {}
	draw(): void {
		if (this.detached) throw new Error(`canvas ${this.id} is detached`);
	}
}

let pool: FakeCanvas[] = [];
let poolSize = 0;
let nextCanvasId = 0;
let framesAvailable = 0;

/** Generators still running; a superseded run must not stay in here. */
let liveRuns = 0;

/** Yields `framesAvailable` frames at 60fps, honouring `poolSize` like the real sink. */
async function* fakeCanvases(startTimestamp = 0) {
	liveRuns++;
	try {
		yield* emitFrames(startTimestamp);
	} finally {
		// MediaBunny closes the run's VideoDecoder in the generator's return
		// path, so "generator finished" is the proxy for "decoder released".
		liveRuns--;
	}
}

async function* emitFrames(startTimestamp: number) {
	for (let i = 0; i < framesAvailable; i++) {
		let canvas: FakeCanvas;
		if (poolSize > 0) {
			const slot = i % poolSize;
			canvas = pool[slot] ?? new FakeCanvas(nextCanvasId++);
			pool[slot] = canvas;
		} else {
			canvas = new FakeCanvas(nextCanvasId++);
		}
		// The sink always writes the decoded frame into the canvas it hands
		// out. This is the step that throws once we've transferred it away.
		canvas.draw();
		yield { canvas, timestamp: startTimestamp + i / 60, duration: 1 / 60 };
	}
}

vi.mock('mediabunny', () => ({
	ALL_FORMATS: [],
	UrlSource: class {
		constructor(readonly url: string) {}
	},
	CanvasSink: class {
		constructor(
			_track: unknown,
			readonly options: { poolSize?: number },
		) {
			poolSize = options?.poolSize ?? 0;
			pool = [];
		}
		canvases(startTimestamp?: number) {
			return fakeCanvases(startTimestamp ?? 0);
		}
		async getCanvas(timestamp: number) {
			const canvas = new FakeCanvas(nextCanvasId++);
			return { canvas, timestamp, duration: 1 / 60 };
		}
	},
	Input: class {
		async canRead() {
			return true;
		}
		async getPrimaryVideoTrack() {
			return {
				getCodedWidth: async () => 1920,
				getCodedHeight: async () => 1080,
				computePacketStats: async () => ({ averagePacketRate: 60 }),
			};
		}
		async computeDuration() {
			return 60;
		}
		dispose() {}
	},
}));

type Posted = { msg: { type: string; [k: string]: unknown }; transfer: unknown[] };

describe('decode run against MediaBunny canvas semantics', () => {
	let posted: Posted[];
	let onmessage: ((e: { data: unknown }) => void) | null;

	beforeEach(() => {
		posted = [];
		onmessage = null;
		nextCanvasId = 0;
		framesAvailable = 30;
		liveRuns = 0;
		const fakeSelf = {
			postMessage(msg: unknown, transfer: unknown[] = []) {
				posted.push({ msg: msg as Posted['msg'], transfer });
				// A transferred canvas is detached in the sender, exactly as
				// the structured-clone algorithm does it.
				for (const t of transfer) if (t instanceof FakeCanvas) t.detached = true;
			},
			set onmessage(h: (e: { data: unknown }) => void) {
				onmessage = h;
			},
			get onmessage() {
				return onmessage as (e: { data: unknown }) => void;
			},
		};
		vi.stubGlobal('self', fakeSelf);
	});

	afterEach(() => {
		vi.unstubAllGlobals();
		vi.resetModules();
	});

	async function boot() {
		const { startMediabunnyWorker } = await import('../src/playback/worker');
		startMediabunnyWorker();
		onmessage?.({ data: { type: 'init', url: 'asset://x.mp4' } });
		await vi.waitFor(() => expect(posted.some((p) => p.msg.type === 'ready')).toBe(true));
	}

	it('streams every frame without the sink drawing into a transferred canvas', async () => {
		await boot();
		onmessage?.({ data: { type: 'seek', seq: 1, originalSec: 0 } });
		await vi.waitFor(() =>
			expect(posted.filter((p) => p.msg.type === 'frame').length).toBe(framesAvailable),
		);
		const errors = posted.filter((p) => p.msg.type === 'error');
		expect(errors, `decode run died: ${JSON.stringify(errors[0]?.msg)}`).toEqual([]);
	});

	it('does not ask the sink to pool canvases it transfers away', async () => {
		await boot();
		onmessage?.({ data: { type: 'seek', seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(posted.some((p) => p.msg.type === 'frame')).toBe(true));
		// Pooling + transfer is the incompatibility itself; the fix is to not
		// pool, so assert the sink was constructed without one.
		expect(poolSize).toBe(0);
	});

	it('releases a run parked on backpressure when a seek supersedes it', async () => {
		// Enough frames that the run parks on the 0.75s lookahead instead of
		// ending on its own — that park is where the old decoder got stranded.
		framesAvailable = 600;
		await boot();
		onmessage?.({ data: { type: 'seek', seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(liveRuns).toBe(1));
		await vi.waitFor(() =>
			expect(posted.filter((p) => p.msg.type === 'frame').length).toBeGreaterThan(45),
		);

		// Scrub elsewhere without ever sending a `playhead` — exactly what a
		// paused click-to-click scrub does.
		onmessage?.({ data: { type: 'seek', seq: 2, originalSec: 30 } });
		await vi.waitFor(
			() => expect(liveRuns).toBe(1),
			{ timeout: 2000 },
		);
	});

	it('posts each frame under its real presentation timestamp', async () => {
		await boot();
		onmessage?.({ data: { type: 'seek', seq: 1, originalSec: 0 } });
		await vi.waitFor(() =>
			expect(posted.filter((p) => p.msg.type === 'frame').length).toBe(framesAvailable),
		);
		const stamps = posted.filter((p) => p.msg.type === 'frame').map((p) => p.msg.originalSec);
		expect(stamps[0]).toBeCloseTo(0, 6);
		expect(stamps[1]).toBeCloseTo(1 / 60, 6);
		expect(stamps.every((s, i) => i === 0 || (s as number) > (stamps[i - 1] as number))).toBe(true);
	});
});
