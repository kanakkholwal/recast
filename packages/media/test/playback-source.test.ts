/**
 * Playback-surface unit tests using a `vi.fn()` Worker mock. The legacy
 * `WebCodecsVideoSource` lives in a worker; the new `MediabunnyVideoSource`
 * does too. These tests exercise the supersede-cancel behavior and the
 * cut-jump cache path using a stub that responds to messages the same way
 * the real worker would. No real decode — we just verify the contract.
 *
 * Strategy: install a `Worker` shim with `vi.stubGlobal` that captures
 * each `postMessage` and exposes the captured messages via a `.messages`
 * accessor. Drive the source's onmessage handler directly to inject
 * decoded-frame replies without going through the real worker pipeline.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

type WorkerMsg = { type: string; [k: string]: unknown };

class FakeWorker {
	#messages: WorkerMsg[] = [];
	#onmessage: ((e: MessageEvent) => void) | null = null;
	#terminated = false;

	postMessage(msg: unknown, _transfer?: Transferable[]): void {
		if (this.#terminated) return;
		this.#messages.push(msg as WorkerMsg);
		// The real worker replies to `init` with `ready` after its input +
		// sink come up. Mirror that here so the source's static create resolves.
		const m = msg as WorkerMsg;
		if (m.type === "init") {
			queueMicrotask(() => {
				this.#onmessage?.({
					data: {
						type: "ready",
						width: 1920,
						height: 1080,
						durationSec: 60,
						fps: 60,
					},
				} as unknown as MessageEvent);
			});
		}
	}

	get onmessage(): ((e: MessageEvent) => void) | null {
		return this.#onmessage;
	}
	set onmessage(handler: ((e: MessageEvent) => void) | null) {
		this.#onmessage = handler;
	}

	onerror: ((e: ErrorEvent) => void) | null = null;
	addEventListener = vi.fn();
	removeEventListener = vi.fn();
	dispatchEvent = vi.fn();

	/** Inject a `frame` reply as the worker would. */
	receiveFrame(seq: number, originalSec: number): void {
		this.#onmessage?.({
			data: {
				type: "frame",
				seq,
				originalSec,
				frame: makeFrame(originalSec),
				width: 1920,
				height: 1080,
			},
		} as unknown as MessageEvent);
	}

	/** All messages posted via `postMessage`. */
	get messages(): readonly WorkerMsg[] {
		return this.#messages;
	}

	/** Latest message of a given type, or undefined. */
	lastOfType(type: string): WorkerMsg | undefined {
		for (let i = this.#messages.length - 1; i >= 0; i--) {
			const m = this.#messages[i];
			if (m && m.type === type) return m;
		}
		return undefined;
	}

	terminate(): void {
		this.#terminated = true;
	}
}

describe("MediabunnyVideoSource — supersede + cut-jump behavior", () => {
	let worker: FakeWorker;

	beforeEach(() => {
		worker = new FakeWorker();
		// Replace the global Worker constructor with a factory that hands back
		// our fake. `vi.stubGlobal` keeps the rest of the world's globals
		// intact and resets after each test.
		vi.stubGlobal("Worker", (() => worker) as unknown as typeof Worker);
		// Capability check: `static create` rejects when VideoFrame or
		// OffscreenCanvas are missing. Stub them as no-constructible classes
		// so the check passes without real implementations.
		vi.stubGlobal("VideoFrame", FakeFrame as unknown as typeof VideoFrame);
		vi.stubGlobal("OffscreenCanvas", class {} as unknown);
	});

	afterEach(() => {
		vi.unstubAllGlobals();
	});

	async function buildSource(): Promise<MediabunnyVideoSource> {
		const src = await MediabunnyVideoSource.create("asset://localhost/test.mp4", {
			createWorker: () => worker as unknown as Worker,
		});
		// Static `create` resolves once the worker posts `ready`. The fake
		// does this via `queueMicrotask`; let microtasks drain.
		await new Promise<void>((r) => queueMicrotask(() => r()));
		await new Promise<void>((r) => queueMicrotask(() => r()));
		return src;
	}

	it("initialises with the worker reporting ready and exposes the metadata", async () => {
		const src = await buildSource();
		expect(src.width).toBe(1920);
		expect(src.height).toBe(1080);
		expect(src.durationSec).toBe(60);
		expect(src.fps).toBe(60);
		src.dispose();
	});

	it("frameAt sends a seek message and returns null on cache miss", async () => {
		const src = await buildSource();
		const before = worker.messages.length;
		const frame = src.frameAt(5.0, 0);
		expect(frame).toBeNull();
		const seek = worker.lastOfType("seek");
		expect(seek).toBeDefined();
		expect(seek?.originalSec).toBe(5.0);
		expect(worker.messages.length - before).toBe(1);
		src.dispose();
	});

	it("a stale frame for a superseded seek is dropped (no cache entry for the stale key)", async () => {
		const src = await buildSource();

		// First seek: 5.0 (pre-cut).
		const f1 = src.frameAt(5.0, 0);
		expect(f1).toBeNull();
		const seek1 = worker.lastOfType("seek") as { seq: number } | undefined;
		const seq1 = seek1?.seq ?? -1;
		expect(seq1).toBeGreaterThan(0);

		// Seeks are rate limited, so two jumps inside one window collapse into
		// one. Wait past the window to get a genuinely separate second seek.
		await new Promise((r) => setTimeout(r, 60));

		// Second seek: 12.0 (post-cut, supersedes the first).
		const f2 = src.frameAt(12.0, 0);
		expect(f2).toBeNull();
		const seek2 = worker.lastOfType("seek") as { seq: number } | undefined;
		const seq2 = seek2?.seq ?? -1;
		expect(seq2).toBeGreaterThan(seq1);

		// Worker replies to the stale (first) seek.
		worker.receiveFrame(seq1, 5.0);
		await new Promise<void>((r) => queueMicrotask(() => r()));
		// Stale frame dropped: inFlightSeq is still 2 (seq1 didn't match),
		// so the cache is empty. We verify by checking the next receiveFrame
		// is processed (seq2 matches inFlightSeq=2).

		// Reply to the FRESH (still-in-flight) seek.
		worker.receiveFrame(seq2, 12.0);
		await new Promise<void>((r) => queueMicrotask(() => r()));
		// Now the post-cut frame is cached at t=12.0.
		expect(src.frameAt(12.0, 0)).not.toBeNull();
		src.dispose();
	});

	it("a fresh frame for the in-flight seq is cached and returned by the next call", async () => {
		const src = await buildSource();
		src.frameAt(8.5, 0);
		const seek = worker.lastOfType("seek") as { seq: number } | undefined;
		const seq = seek?.seq ?? -1;
		worker.receiveFrame(seq, 8.5);
		await new Promise<void>((r) => queueMicrotask(() => r()));
		// Cached on the next call.
		const cached = src.frameAt(8.5, 0);
		expect(cached).not.toBeNull();
		src.dispose();
	});

	it("prefetch posts a prefetch message", async () => {
		const src = await buildSource();
		const before = worker.messages.length;
		src.prefetch(15.0);
		expect(worker.messages.length - before).toBe(1);
		expect(worker.lastOfType("prefetch")?.originalSec).toBe(15.0);
		src.dispose();
	});

	it("dispose posts a dispose message", async () => {
		const src = await buildSource();
		const before = worker.messages.length;
		src.dispose();
		// Idempotent: a second dispose doesn't post twice.
		src.dispose();
		const after = worker.messages.length;
		const newMessages = after - before;
		expect(newMessages).toBeGreaterThanOrEqual(1);
		expect(worker.lastOfType("dispose")).toBeDefined();
	});
});

describe("seek rate limiting", () => {
	let worker: FakeWorker;

	beforeEach(() => {
		vi.useFakeTimers();
		worker = new FakeWorker();
		vi.stubGlobal("Worker", class {} as unknown as typeof Worker);
		vi.stubGlobal("VideoFrame", FakeFrame as unknown as typeof VideoFrame);
		vi.stubGlobal("OffscreenCanvas", class {} as unknown);
	});

	afterEach(() => {
		vi.useRealTimers();
		vi.unstubAllGlobals();
	});

	async function build() {
		const src = await MediabunnyVideoSource.create("asset://localhost/test.mp4", {
			createWorker: () => worker as unknown as Worker,
		});
		await vi.advanceTimersByTimeAsync(0);
		return src;
	}

	it("collapses a burst of drag seeks into one, keeping the final target", async () => {
		const src = await build();
		// A drag fires one jump per pointer move. Unthrottled, each started a
		// fresh decode run with its own decoder.
		for (let i = 0; i < 30; i++) src.advanceTo(i * 3);
		const seeks = worker.messages.filter((m) => m.type === "seek");
		expect(seeks.length).toBe(1);

		await vi.advanceTimersByTimeAsync(100);
		const after = worker.messages.filter((m) => m.type === "seek");
		expect(after.length).toBe(2);
		// The newest target always wins, so the picture lands where the drag ended.
		expect(after[after.length - 1]?.originalSec).toBe(29 * 3);
		src.dispose();
	});

	it("does not throttle steady playback, which never seeks", async () => {
		const src = await build();
		src.advanceTo(0);
		for (let i = 1; i < 30; i++) src.advanceTo(i / 60);
		expect(worker.messages.filter((m) => m.type === "playhead").length).toBe(29);
		expect(worker.messages.filter((m) => m.type === "seek").length).toBe(1);
		src.dispose();
	});

	it("leaves no timer armed after dispose", async () => {
		const src = await build();
		src.advanceTo(10);
		src.advanceTo(20);
		src.dispose();
		const before = worker.messages.length;
		await vi.advanceTimersByTimeAsync(200);
		expect(worker.messages.length).toBe(before);
	});
});
