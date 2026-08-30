import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Drives the REAL decode worker against a mocked MediaBunny. The source-side
 * harness could never catch this class of bug: it stubs the worker, so anything
 * about how the worker uses the sink was invisible to it.
 *
 * The mock enforces MediaBunny's actual ownership contract — `toVideoFrame()`
 * hands out a frame that must be closed separately from the sample it came
 * from, and both are limited decoder surfaces.
 */

class FakeVideoFrame {
	closed = false;
	constructor(
		readonly codedWidth: number,
		readonly codedHeight: number,
	) {}
	close() {
		this.closed = true;
	}
}

class FakeSample {
	closed = false;
	readonly frames: FakeVideoFrame[] = [];
	constructor(
		readonly timestamp: number,
		private readonly w: number,
		private readonly h: number,
	) {}
	toVideoFrame(): FakeVideoFrame {
		const f = new FakeVideoFrame(this.w, this.h);
		this.frames.push(f);
		openFrames.add(f);
		return f;
	}
	close() {
		this.closed = true;
		openSamples.delete(this);
	}
}

let openSamples = new Set<FakeSample>();
let openFrames = new Set<FakeVideoFrame>();
let framesAvailable = 0;
let canDecode = true;
let videoWidth = 1920;
let videoHeight = 1080;
let liveRuns = 0;
/** Highest number of decode runs alive at once — each holds its own decoder. */
let peakRuns = 0;
/** Total runs started; a restart means a cold decoder, so these are not free. */
let runsStarted = 0;

/** Decoder startup before the first sample appears. That real cost is why a
 *  superseded run cannot notice it has been replaced. */
const DECODER_STARTUP_MS = 25;

/**
 * Mirrors MediaBunny's hand-rolled iterator rather than a native async
 * generator. The distinction is the whole point: its `return()` marks the run
 * terminated IMMEDIATELY, even while `next()` is still awaiting decoder
 * startup. A native generator defers that until the pending await resolves,
 * which hides the bug this models.
 */
function fakeSamples(startTimestamp = 0) {
	liveRuns++;
	runsStarted++;
	if (liveRuns > peakRuns) peakRuns = liveRuns;
	let i = 0;
	let terminated = false;
	const release = () => {
		if (terminated) return;
		terminated = true;
		liveRuns--;
	};
	return {
		async next() {
			if (i === 0) await new Promise((r) => setTimeout(r, DECODER_STARTUP_MS));
			if (terminated || i >= framesAvailable) {
				release();
				return { value: undefined as unknown as FakeSample, done: true };
			}
			const s = new FakeSample(startTimestamp + i / 60, videoWidth, videoHeight);
			openSamples.add(s);
			i++;
			return { value: s, done: false };
		},
		async return() {
			release();
			return { value: undefined as unknown as FakeSample, done: true };
		},
		[Symbol.asyncIterator]() {
			return this;
		},
	};
}

vi.mock("mediabunny", () => ({
	ALL_FORMATS: [],
	UrlSource: class {
		constructor(readonly url: string) {}
	},
	BlobSource: class {
		constructor(readonly blob: Blob) {}
	},
	VideoSampleSink: class {
		samples(startTimestamp?: number) {
			return fakeSamples(startTimestamp ?? 0);
		}
		async getSample(timestamp: number) {
			const s = new FakeSample(timestamp, videoWidth, videoHeight);
			openSamples.add(s);
			return s;
		}
	},
	Input: class {
		async canRead() {
			return true;
		}
		async getPrimaryVideoTrack() {
			return {
				getCodedWidth: async () => videoWidth,
				getCodedHeight: async () => videoHeight,
				computePacketStats: async () => ({ averagePacketRate: 60 }),
				canDecode: async () => canDecode,
				getCodec: async () => "hevc",
			};
		}
		async computeDuration() {
			return 60;
		}
		dispose() {}
	},
}));

type Posted = { msg: { type: string; [k: string]: unknown }; transfer: unknown[] };

describe("decode run against MediaBunny sample semantics", () => {
	let posted: Posted[];
	let onmessage: ((e: { data: unknown }) => void) | null;

	beforeEach(() => {
		posted = [];
		onmessage = null;
		framesAvailable = 30;
		canDecode = true;
		videoWidth = 1920;
		videoHeight = 1080;
		liveRuns = 0;
		peakRuns = 0;
		runsStarted = 0;
		openSamples = new Set();
		openFrames = new Set();
		const fakeSelf = {
			postMessage(msg: unknown, transfer: unknown[] = []) {
				posted.push({ msg: msg as Posted["msg"], transfer });
			},
			set onmessage(h: (e: { data: unknown }) => void) {
				onmessage = h;
			},
			get onmessage() {
				return onmessage as (e: { data: unknown }) => void;
			},
		};
		vi.stubGlobal("self", fakeSelf);
	});

	afterEach(async () => {
		// Stop any run still in flight, or it posts frames into the next test's buffer: the module outlives per-test globals.
		onmessage?.({ data: { type: "dispose" } });
		await new Promise((r) => setTimeout(r, DECODER_STARTUP_MS * 2));
		vi.unstubAllGlobals();
		vi.resetModules();
	});

	async function boot() {
		const { startMediabunnyWorker } = await import("../src/playback/worker");
		startMediabunnyWorker();
		onmessage?.({ data: { type: "init", src: { kind: "url", url: "asset://x.mp4" } } });
		await vi.waitFor(() => expect(posted.some((p) => p.msg.type === "ready")).toBe(true));
	}

	const frames = () => posted.filter((p) => p.msg.type === "frame");

	it("streams frames and transfers each one", async () => {
		await boot();
		onmessage?.({ data: { type: "seek", seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(frames().length).toBeGreaterThan(0));
		expect(posted.filter((p) => p.msg.type === "error")).toEqual([]);
		// Every frame must be in its message's transfer list, or it is structured-cloned instead of moved.
		for (const p of frames()) expect(p.transfer).toContain(p.msg.frame);
	});

	it("closes every sample it takes a frame from", async () => {
		await boot();
		onmessage?.({ data: { type: "seek", seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(frames().length).toBeGreaterThan(2));
		await new Promise((r) => setTimeout(r, 100));
		// A sample holds its own decode surface, and leaking them exhausts the pool.
		expect([...openSamples].filter((s) => !s.closed)).toEqual([]);
	});

	it("bounds decode-ahead by the frame budget, not a fixed duration", async () => {
		// 4K: a fixed 0.75s lookahead decoded ~45 frames per window against a 4-frame cache, and the churn killed the renderer.
		videoWidth = 3840;
		videoHeight = 2160;
		framesAvailable = 600;
		await boot();
		onmessage?.({ data: { type: "seek", seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(frames().length).toBeGreaterThan(0));
		// Let it run well past any short-lived burst, then confirm it parked.
		await new Promise((r) => setTimeout(r, 150));
		expect(frames().length).toBeLessThan(12);
	});

	it("rejects an undecodable codec at init instead of on the first decode", async () => {
		canDecode = false;
		const { startMediabunnyWorker } = await import("../src/playback/worker");
		startMediabunnyWorker();
		onmessage?.({ data: { type: "init", src: { kind: "url", url: "asset://x.mp4" } } });
		await vi.waitFor(() => expect(posted.some((p) => p.msg.type === "error")).toBe(true));
		const err = posted.find((p) => p.msg.type === "error")?.msg;
		expect(err?.code).toBe("unsupported");
		expect(String(err?.message)).toContain("hevc");
		expect(posted.some((p) => p.msg.type === "ready")).toBe(false);
	});

	it("releases a run parked on backpressure when a seek supersedes it", async () => {
		framesAvailable = 600;
		await boot();
		onmessage?.({ data: { type: "seek", seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(liveRuns).toBe(1));
		await vi.waitFor(() => expect(frames().length).toBeGreaterThan(0));

		// Scrub elsewhere without ever sending a `playhead`, exactly what a paused click-to-click scrub does.
		onmessage?.({ data: { type: "seek", seq: 2, originalSec: 30 } });
		await vi.waitFor(() => expect(liveRuns).toBe(1), { timeout: 2000 });
	});

	it("tears down the previous run before starting the next", async () => {
		// A superseded run blocks in `for await` until its first sample, so waiting left one live decoder per scrub tick.
		framesAvailable = 600;
		await boot();
		for (let i = 0; i < 20; i++) {
			onmessage?.({ data: { type: "seek", seq: i + 1, originalSec: i * 5 } });
		}
		await vi.waitFor(() => expect(frames().length).toBeGreaterThan(0), { timeout: 2000 });
		// Peak, not eventual: every run alive at one instant holds its own decoder, and a drag issues a seek per move.
		expect(peakRuns).toBeLessThanOrEqual(2);
	});

	it("absorbs a seek the live run is already streaming through", async () => {
		// Restarting for a target the run already decoded pays full decoder startup and freezes the picture until the next drag.
		framesAvailable = 600;
		await boot();
		onmessage?.({ data: { type: "seek", seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(frames().length).toBeGreaterThan(3));
		const runsBefore = runsStarted;
		const covered = frames()[2]?.msg.originalSec as number;
		onmessage?.({ data: { type: "seek", seq: 2, originalSec: covered } });
		await new Promise((r) => setTimeout(r, DECODER_STARTUP_MS * 3));
		expect(runsStarted).toBe(runsBefore);
	});

	it("restarts once the run that covered a target has ended", async () => {
		// Coverage must die with the run, or a seek inside its old window is absorbed and no frame is ever decoded.
		framesAvailable = 4;
		await boot();
		onmessage?.({ data: { type: "seek", seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(liveRuns).toBe(0), { timeout: 2000 });
		const runsBefore = runsStarted;
		onmessage?.({ data: { type: "seek", seq: 2, originalSec: 1 / 60 } });
		await vi.waitFor(() => expect(runsStarted).toBe(runsBefore + 1), { timeout: 2000 });
	});

	it("posts each frame under its real presentation timestamp", async () => {
		await boot();
		onmessage?.({ data: { type: "seek", seq: 1, originalSec: 0 } });
		await vi.waitFor(() => expect(frames().length).toBeGreaterThan(2));
		const stamps = frames().map((p) => p.msg.originalSec as number);
		expect(stamps[0]).toBeCloseTo(0, 6);
		expect(stamps[1]).toBeCloseTo(1 / 60, 6);
		expect(stamps.every((s, i) => i === 0 || s > (stamps[i - 1] as number))).toBe(true);
	});
});
