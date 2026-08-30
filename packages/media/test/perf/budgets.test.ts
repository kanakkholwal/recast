import { describe, expect, it } from "vitest";
import { FrameCache } from "../../src/cache";
import type { CachedFrame } from "../../src/cache/storage";

/**
 * Budgets from REQUIREMENTS.md §3. Block 1 declares the table, block 2
 * enforces the rows checkable without real media; the rest are listed below.
 */
const BUDGETS = {
	// TTFF
	ttff4kMs: 800,
	ttff1080pMs: 200,
	// Scrub
	scrubCachedP95Ms: 50,
	scrubColdP95Ms: 200,
	// Playback
	frameToGlassP95Ms: 16.7,
	cutCrossP95Ms: 250,
	inpPlaybackP95Ms: 100,
	// Memory
	decodedFrameCapBytes: 512 * 1024 * 1024,
	// Bundle (gated in ./bundle.test.ts)
	desktopBundleGzKb: 80,
	webPageGzKb: 5,
	conversionWorkerGzKb: 220,
	// Sync
	audioSyncDriftMsPer10Min: 10,
} as const;

describe("perf budgets (REQUIREMENTS.md §3 — non-negotiable)", () => {
	it("declares the TTFF budget for 4K @ 60fps", () => {
		expect(BUDGETS.ttff4kMs).toBe(800);
	});

	it("declares the TTFF budget for 1080p @ 30fps", () => {
		expect(BUDGETS.ttff1080pMs).toBe(200);
	});

	it("declares the cached scrub latency p95", () => {
		expect(BUDGETS.scrubCachedP95Ms).toBe(50);
	});

	it("declares the cold scrub latency p95", () => {
		expect(BUDGETS.scrubColdP95Ms).toBe(200);
	});

	it("declares the frame-to-glass p95", () => {
		expect(BUDGETS.frameToGlassP95Ms).toBeLessThanOrEqual(16.7);
	});

	it("declares the cut-cross latency p95", () => {
		expect(BUDGETS.cutCrossP95Ms).toBe(250);
	});

	it("declares the playback INP p95", () => {
		expect(BUDGETS.inpPlaybackP95Ms).toBe(100);
	});

	it("declares the decoded-frame memory cap", () => {
		expect(BUDGETS.decodedFrameCapBytes).toBe(512 * 1024 * 1024);
	});

	it("declares the desktop bundle budget (gz)", () => {
		expect(BUDGETS.desktopBundleGzKb).toBe(80);
	});

	it("declares the web page budget (gz)", () => {
		expect(BUDGETS.webPageGzKb).toBe(5);
	});

	it("declares the conversion worker budget (gz, on-demand chunk)", () => {
		expect(BUDGETS.conversionWorkerGzKb).toBe(220);
	});

	it("declares the audio sync drift budget over 10 min", () => {
		expect(BUDGETS.audioSyncDriftMsPer10Min).toBeLessThanOrEqual(10);
	});
});

/**
 * Enforced rows. Every assertion here MUST exercise real package code — if it
 * only compares `BUDGETS.x` to a literal it belongs in the block above.
 */
describe("perf budgets — enforced against real code", () => {
	function frame(w: number, h: number) {
		return { width: w, height: h, close: () => undefined } as unknown as CachedFrame;
	}

	it("FrameCache defaults to the §3 decoded-frame memory cap", () => {
		expect(new FrameCache().memoryCapBytes).toBe(BUDGETS.decodedFrameCapBytes);
	});

	it("the decoded-frame cap is actually enforced on write", () => {
		// 1 MB frames, 8 MB cap → at most 8 resident.
		const oneMb = 512 * 512 * 4;
		const cache = new FrameCache({ memoryCapBytes: oneMb * 8 });
		for (let i = 0; i < 64; i++) cache.write(i, frame(512, 512));
		const stats = cache.cacheStats();
		expect(stats.bytes).toBeLessThanOrEqual(oneMb * 8);
		expect(stats.entryCount).toBeLessThanOrEqual(8);
		expect(stats.evictions).toBeGreaterThanOrEqual(56);
	});

	it("byte accounting never goes NaN (poisons every cap comparison)", () => {
		// `NaN > cap` is false, so a single NaN silently disables the cap.
		const cache = new FrameCache();
		cache.write(1, frame(1920, 1080));
		expect(Number.isNaN(cache.cacheStats().bytes)).toBe(false);
	});
});

// Bundle rows are gated in ./bundle.test.ts; TTFF, scrub p95, frame-to-glass, INP and audio drift still need a browser harness.
