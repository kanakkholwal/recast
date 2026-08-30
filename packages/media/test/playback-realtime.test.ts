/**
 * Simulates continuous 60fps playback against the real source with a worker
 * stub that mirrors the streaming protocol: a `seek` starts a decode run that
 * emits frames at the source rate, and `playhead` releases backpressure.
 *
 * Exists because reasoning about the cache alone proved unreliable — the
 * original exact-key lookup painted 0 of 120 frames while looking correct.
 */

import { afterEach, describe, expect, it, vi } from "vitest";
import { resetFrameCache } from "../src/cache";
import { frameBudget } from "../src/cache/frame-budget";
import { MediabunnyVideoSource } from "../src/playback/source";

/** Stands in for a transferred decode surface. Also installed as the global
 *  `VideoFrame`, so the source's `instanceof` check passes. */
class FakeFrame {
	closed = false;
	readonly codedWidth = 1920;
	readonly codedHeight = 1080;
	constructor(readonly timestamp: number) {}
	close() {
		this.closed = true;
	}
}

function makeFrame(sec: number): FakeFrame {
	return new FakeFrame(Math.round(sec * 1_000_000));
}

/** Outlast the source's seek rate limiter. */
const settleSeekWindow = () => new Promise((r) => setTimeout(r, 60));

const FPS = 30;
const FRAME_SEC = 1 / FPS;
// Decode-ahead is a FRAME budget, not a duration: a fixed 0.75s outran the cache and the picture updated ~6 times in 2s.
const LOOKAHEAD_SEC = frameBudget(1920, 1080).decodeAhead / FPS;

class StreamingWorker {
	onmessage: ((e: MessageEvent) => void) | null = null;
	onerror: ((e: ErrorEvent) => void) | null = null;
	seeks = 0;
	playheads = 0;
	delivered = 0;
	nowMs = 0;
	#playheadSec = 0;
	#run: { seq: number; nextSec: number; readyAtMs: number } | null = null;

	constructor(readonly decodeMs: number) {}

	postMessage(msg: { type: string; seq?: number; originalSec?: number }): void {
		if (msg.type === "init") {
			queueMicrotask(() =>
				this.onmessage?.({
					data: { type: "ready", width: 1920, height: 1080, durationSec: 60, fps: FPS },
				} as MessageEvent),
			);
			return;
		}
		if (msg.type === "seek") {
			this.seeks++;
			this.#playheadSec = msg.originalSec ?? 0;
			this.#run = {
				seq: msg.seq ?? 0,
				nextSec: msg.originalSec ?? 0,
				readyAtMs: this.nowMs + this.decodeMs,
			};
			return;
		}
		if (msg.type === "playhead") {
			this.playheads++;
			this.#playheadSec = msg.originalSec ?? 0;
		}
	}

	/** Advance virtual time; emit every frame whose decode completed. */
	tick(toMs: number): void {
		this.nowMs = toMs;
		while (this.#run && this.#run.readyAtMs <= this.nowMs) {
			// Backpressure: the real worker parks past the lookahead window.
			if (this.#run.nextSec > this.#playheadSec + LOOKAHEAD_SEC) break;
			const sec = this.#run.nextSec;
			this.delivered++;
			this.onmessage?.({
				data: {
					type: "frame",
					seq: this.#run.seq,
					originalSec: sec,
					frame: makeFrame(sec),
					width: 1920,
					height: 1080,
				},
			} as MessageEvent);
			this.#run.nextSec = sec + FRAME_SEC;
			this.#run.readyAtMs = this.nowMs + this.decodeMs;
		}
	}

	terminate(): void {}
	addEventListener = vi.fn();
	removeEventListener = vi.fn();
	dispatchEvent = vi.fn();
}

let worker: StreamingWorker;

describe("continuous playback (60fps rAF)", () => {
	function setup(decodeMs: number) {
		worker = new StreamingWorker(decodeMs);
		vi.stubGlobal("Worker", (() => worker) as unknown as typeof Worker);
		vi.stubGlobal("VideoFrame", FakeFrame as unknown as typeof VideoFrame);
		vi.stubGlobal("OffscreenCanvas", class {} as unknown);
		resetFrameCache();
	}

	afterEach(() => {
		vi.unstubAllGlobals();
		resetFrameCache();
	});

	/** Run `frames` rAF ticks at 60fps against a playhead advancing in real time. */
	async function runPlayback(decodeMs: number, frames = 120) {
		setup(decodeMs);
		const src = await MediabunnyVideoSource.create("asset://x.mp4", {
			createWorker: () => worker as unknown as Worker,
		});
		await new Promise<void>((r) => queueMicrotask(() => r()));
		let painted = 0;
		const distinct = new Set<number>();
		for (let i = 0; i < frames; i++) {
			const tMs = i * (1000 / 60);
			worker.tick(tMs);
			const f = src.frameAt(tMs / 1000);
			if (f) {
				painted++;
				distinct.add((f as unknown as { timestamp: number }).timestamp);
			}
			await Promise.resolve();
		}
		return {
			painted,
			distinct: distinct.size,
			frames,
			src,
			seeks: worker.seeks,
			playheads: worker.playheads,
			delivered: worker.delivered,
		};
	}

	it("paints nearly every frame and issues one seek, not one per rAF", async () => {
		const r = await runPlayback(5, 120);
		expect(r.painted).toBeGreaterThan(110);
		// The old model posted a seek per rAF, each aborting the last.
		expect(r.seeks).toBe(1);
		expect(r.playheads).toBeGreaterThan(100);
	});

	it("advances the picture rather than holding one frame", async () => {
		const r = await runPlayback(5, 120);
		// 120 rAF ticks at 60fps span 2s ≈ 60 frames at 30fps.
		expect(r.distinct).toBeGreaterThan(40);
	});

	it("keeps painting when decode is slower than a frame interval", async () => {
		// 25ms decode used to abort every request and deliver nothing at all.
		const r = await runPlayback(25, 120);
		expect(r.painted).toBeGreaterThan(100);
		expect(r.distinct).toBeGreaterThan(20);
		expect(r.seeks).toBe(1);
	});

	it("reports real throughput instead of hardcoded zeros", async () => {
		const r = await runPlayback(5, 120);
		const stats = r.src.stats();
		expect(stats.avgFps).toBeGreaterThan(0);
		expect(stats.minFps).toBeGreaterThan(0);
	});
});

/**
 * The request policy is what stopped the abort storm: only a genuine jump may
 * restart decode, everything else is backpressure.
 */
describe("seek vs playhead policy", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
		resetFrameCache();
	});

	async function build(decodeMs = 5) {
		worker = new StreamingWorker(decodeMs);
		vi.stubGlobal("Worker", (() => worker) as unknown as typeof Worker);
		vi.stubGlobal("VideoFrame", FakeFrame as unknown as typeof VideoFrame);
		vi.stubGlobal("OffscreenCanvas", class {} as unknown);
		resetFrameCache();
		const src = await MediabunnyVideoSource.create("asset://x.mp4", {
			createWorker: () => worker as unknown as Worker,
		});
		await new Promise<void>((r) => queueMicrotask(() => r()));
		return src;
	}

	it("seeks once on the first request, then rides on playhead updates", async () => {
		const src = await build();
		src.frameAt(0);
		for (let i = 1; i < 20; i++) src.frameAt(i / 60);
		expect(worker.seeks).toBe(1);
		expect(worker.playheads).toBe(19);
	});

	it("seeks again when the playhead jumps backwards (scrub)", async () => {
		const src = await build();
		src.frameAt(5);
		src.frameAt(5.016);
		expect(worker.seeks).toBe(1);
		// Seeks are rate limited so a drag can't rebuild a decoder per move; wait past the window for the next one.
		await settleSeekWindow();
		src.frameAt(1); // scrub back
		expect(worker.seeks).toBe(2);
	});

	it("seeks again on a large forward jump the run cannot reach", async () => {
		const src = await build();
		src.frameAt(0);
		await settleSeekWindow();
		src.frameAt(30);
		expect(worker.seeks).toBe(2);
	});

	it("tolerates small backward jitter without reseeking", async () => {
		const src = await build();
		src.frameAt(5);
		src.frameAt(4.99); // inside FRAME_SLACK_SEC
		expect(worker.seeks).toBe(1);
	});
});
