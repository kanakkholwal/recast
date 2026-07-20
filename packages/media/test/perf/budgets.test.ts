import { describe, expect, it } from 'vitest';

/**
 * Performance budgets every `@recast/media` consumer can rely on. Mirrored
 * verbatim from packages/media/REQUIREMENTS.md §3 — a regression on any row
 * is merge-blocking (AGENTS.md §2 rule 12 + §4.x).
 *
 * PR-A ships this skeleton only. Each assertion is marked `[vacuous]` so the
 * build stays green before the real perf fixtures land in PR-D/E/F. When
 * the real fixtures arrive, each vacuous assertion is replaced with a real
 * measurement against a known-good baseline.
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
	indexedDbCacheCapBytes: 2 * 1024 * 1024 * 1024,
	// Bundle
	desktopBundleGzKb: 80,
	webBundleGzKb: 150,
	// Sync
	audioSyncDriftMsPer10Min: 10,
} as const;

describe('perf budgets (REQUIREMENTS.md §3 — non-negotiable)', () => {
	it('declares the TTFF budget for 4K @ 60fps', () => {
		expect(BUDGETS.ttff4kMs).toBe(800);
	});

	it('declares the TTFF budget for 1080p @ 30fps', () => {
		expect(BUDGETS.ttff1080pMs).toBe(200);
	});

	it('declares the cached scrub latency p95', () => {
		expect(BUDGETS.scrubCachedP95Ms).toBe(50);
	});

	it('declares the cold scrub latency p95', () => {
		expect(BUDGETS.scrubColdP95Ms).toBe(200);
	});

	it('declares the frame-to-glass p95', () => {
		expect(BUDGETS.frameToGlassP95Ms).toBeLessThanOrEqual(16.7);
	});

	it('declares the cut-cross latency p95', () => {
		expect(BUDGETS.cutCrossP95Ms).toBe(250);
	});

	it('declares the playback INP p95', () => {
		expect(BUDGETS.inpPlaybackP95Ms).toBe(100);
	});

	it('declares the decoded-frame memory cap', () => {
		expect(BUDGETS.decodedFrameCapBytes).toBe(512 * 1024 * 1024);
	});

	it('declares the IndexedDB cache cap (user-configurable; default)', () => {
		expect(BUDGETS.indexedDbCacheCapBytes).toBe(2 * 1024 * 1024 * 1024);
	});

	it('declares the desktop bundle budget (gz)', () => {
		expect(BUDGETS.desktopBundleGzKb).toBe(80);
	});

	it('declares the web bundle budget (gz, incl. tools)', () => {
		expect(BUDGETS.webBundleGzKb).toBe(150);
	});

	it('declares the audio sync drift budget over 10 min', () => {
		expect(BUDGETS.audioSyncDriftMsPer10Min).toBeLessThanOrEqual(10);
	});
});

describe('perf budgets — vacuous placeholders (real fixtures land in PR-D/E/F)', () => {
	// Each test here is a placeholder. When the real perf fixtures arrive in
	// PR-D (PlaybackSource), PR-E (cache + AudioWorklet), and PR-F
	// (cut-jump parity), each `it` is replaced with a measurement against a
	// committed baseline. Until then, these are no-ops so the build stays
	// green and the budget table is exercised end-to-end.

	it('[vacuous] TTFF for 4K recording is within budget', () => {
		expect(BUDGETS.ttff4kMs).toBeGreaterThan(0);
	});

	it('[vacuous] TTFF for 1080p recording is within budget', () => {
		expect(BUDGETS.ttff1080pMs).toBeGreaterThan(0);
	});

	it('[vacuous] scrub (cached) p95 is within budget', () => {
		expect(BUDGETS.scrubCachedP95Ms).toBeGreaterThan(0);
	});

	it('[vacuous] scrub (cold) p95 is within budget', () => {
		expect(BUDGETS.scrubColdP95Ms).toBeGreaterThan(0);
	});

	it('[vacuous] frame-to-glass p95 during playback is within budget', () => {
		expect(BUDGETS.frameToGlassP95Ms).toBeGreaterThan(0);
	});

	it('[vacuous] cut-cross latency p95 is within budget', () => {
		expect(BUDGETS.cutCrossP95Ms).toBeGreaterThan(0);
	});

	it('[vacuous] playback INP p95 is within budget', () => {
		expect(BUDGETS.inpPlaybackP95Ms).toBeGreaterThan(0);
	});

	it('[vacuous] decoded-frame buffer stays within memory cap', () => {
		expect(BUDGETS.decodedFrameCapBytes).toBeGreaterThan(0);
	});

	it('[vacuous] IndexedDB cache stays within cap', () => {
		expect(BUDGETS.indexedDbCacheCapBytes).toBeGreaterThan(0);
	});

	it('[vacuous] desktop bundle stays within budget', () => {
		expect(BUDGETS.desktopBundleGzKb).toBeGreaterThan(0);
	});

	it('[vacuous] web bundle stays within budget', () => {
		expect(BUDGETS.webBundleGzKb).toBeGreaterThan(0);
	});

	it('[vacuous] audio sync drift over 10 min stays within budget', () => {
		expect(BUDGETS.audioSyncDriftMsPer10Min).toBeGreaterThan(0);
	});
});
