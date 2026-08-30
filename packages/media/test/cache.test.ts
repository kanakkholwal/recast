import { describe, expect, it } from "vitest";
import { FrameCache, resetFrameCache, setFrameCache } from "../src/cache";
import { estimateFrameBytes } from "../src/cache/storage";
import type { CachedFrame } from "../src/cache/storage";

/**
 * Minimal `CachedFrame` stub. Real frames carry GPU resources; tests
 * just need an object with `width`/`height` and an idempotent `.close()`.
 */
function fakeFrame(w: number, h: number, onClose?: () => void): CachedFrame {
	return {
		width: w,
		height: h,
		close: () => onClose?.(),
	} as unknown as CachedFrame;
}

describe("estimateFrameBytes", () => {
	it("uses width × height × 4 (RGBA)", () => {
		expect(estimateFrameBytes(fakeFrame(1920, 1080))).toBe(1920 * 1080 * 4);
		expect(estimateFrameBytes(fakeFrame(640, 480))).toBe(640 * 480 * 4);
	});
});

describe("FrameCache", () => {
	it("serves what it wrote", () => {
		const cache = new FrameCache();
		const frame = fakeFrame(640, 360);
		cache.write(1_000_000, frame);
		expect(cache.readMemory(1_000_000)).toBe(frame);
	});

	it("evictCache drops and closes everything", () => {
		const cache = new FrameCache();
		let closed = 0;
		cache.write(
			1,
			fakeFrame(640, 360, () => closed++),
		);
		cache.write(
			2,
			fakeFrame(640, 360, () => closed++),
		);
		expect(cache.evictCache()).toBe(2);
		expect(cache.readMemory(1)).toBeNull();
		expect(cache.readMemory(2)).toBeNull();
		expect(closed).toBe(2);
	});

	it("cacheStats reports live usage against the cap", () => {
		const cache = new FrameCache();
		cache.write(1, fakeFrame(640, 360)); // 921,600
		cache.write(2, fakeFrame(640, 360));
		const stats = cache.cacheStats();
		expect(stats.entryCount).toBe(2);
		expect(stats.bytes).toBe(2 * 640 * 360 * 4);
		expect(stats.capBytes).toBeGreaterThan(stats.bytes);
	});
});

/**
 * Frame-lifetime regressions; each fails against the pre-fix cache. They were
 * invisible because every fixture was ImageBitmap-shaped, unlike production.
 */
describe("frame lifetime (REQUIREMENTS.md §3 memory cap, §5 ownership)", () => {
	/** `VideoFrame`-shaped stub: coded* dimensions, no width/height. */
	function fakeVideoFrame(w: number, h: number, onClose?: () => void) {
		return {
			codedWidth: w,
			codedHeight: h,
			displayWidth: w,
			displayHeight: h,
			close: () => onClose?.(),
		};
	}

	it("estimateFrameBytes handles VideoFrame dimensions (was NaN)", () => {
		// `NaN > cap` is false, so one NaN silently disabled both caps.
		const bytes = estimateFrameBytes(fakeVideoFrame(1920, 1080) as never);
		expect(Number.isNaN(bytes)).toBe(false);
		expect(bytes).toBe(1920 * 1080 * 4);
	});

	it("charges a large fallback (never 0) when dimensions are non-finite", () => {
		// A 0-byte estimate never adds to the total, so the cap always passes, eviction stops and the Map grows unbounded.
		const bytes = estimateFrameBytes(fakeVideoFrame(Number.NaN, Number.NaN) as never);
		expect(bytes).toBeGreaterThan(0);
	});

	it("caps the in-memory layer and closes the frames it evicts", () => {
		const frameBytes = 640 * 360 * 4; // 921,600
		// Room for exactly 2 frames.
		const cache = new FrameCache({ memoryCapBytes: frameBytes * 2 });
		const closed: number[] = [];
		for (let i = 0; i < 5; i++) {
			cache.write(
				i,
				fakeFrame(640, 360, () => closed.push(i)),
			);
		}
		// Without a cap the Map grew forever and nothing was ever closed.
		expect(cache.readMemory(0)).toBeNull();
		expect(cache.readMemory(4)).not.toBeNull();
		expect(closed).toContain(0);
		expect(closed.length).toBe(3);
	});

	it("lowering memoryCapBytes evicts immediately", () => {
		const cache = new FrameCache({ memoryCapBytes: 64 * 1024 * 1024 });
		for (let i = 0; i < 4; i++) cache.write(i, fakeFrame(640, 360));
		expect(cache.readMemory(3)).not.toBeNull();
		cache.memoryCapBytes = 640 * 360 * 4; // room for one
		expect(cache.readMemory(0)).toBeNull();
		expect(cache.readMemory(3)).not.toBeNull();
	});

	it("closes the outgoing frame when a key is overwritten", () => {
		const cache = new FrameCache();
		let closed = false;
		cache.write(
			1,
			fakeFrame(64, 64, () => (closed = true)),
		);
		cache.write(1, fakeFrame(64, 64));
		expect(closed).toBe(true);
	});

	it("setScope closes held frames instead of dropping the Map", () => {
		const cache = new FrameCache();
		let closed = false;
		cache.write(
			1,
			fakeFrame(64, 64, () => (closed = true)),
		);
		cache.setScope("recording-b");
		expect(closed).toBe(true);
	});

	it("reports bytes under the cap and an eviction count", () => {
		const frameBytes = 640 * 360 * 4;
		const cache = new FrameCache({ memoryCapBytes: frameBytes * 2 });
		for (let i = 0; i < 4; i++) cache.write(i, fakeFrame(640, 360));
		const stats = cache.cacheStats();
		expect(Number.isNaN(stats.bytes)).toBe(false);
		expect(stats.bytes).toBeLessThanOrEqual(stats.capBytes);
		expect(stats.evictions).toBeGreaterThan(0);
	});
});

describe("cache factory", () => {
	it("resetFrameCache clears the singleton", () => {
		setFrameCache(new FrameCache());
		resetFrameCache();
		// After reset, getFrameCache() lazily rebuilds; the call must simply not throw.
		expect(() => resetFrameCache()).not.toThrow();
	});
});
/**
 * `readNearest` is what playback actually calls. Frame timestamps land on
 * presentation times while the render loop asks for arbitrary microseconds, so
 * an exact-match lookup misses every time — the bug that painted 0/120 frames.
 */
describe("FrameCache.readNearest", () => {
	function seeded() {
		const cache = new FrameCache();
		// Frames at 0ms, 33ms, 66ms, 99ms (µs keys).
		for (const ms of [0, 33, 66, 99]) cache.write(ms * 1000, fakeFrame(64, 64));
		return cache;
	}

	it("returns the newest frame at or before the asked time", () => {
		const cache = seeded();
		expect(cache.readNearest(50_000)).toBe(cache.readMemory(33_000));
		expect(cache.readNearest(99_999)).toBe(cache.readMemory(99_000));
	});

	it("returns an exact hit when the key matches", () => {
		const cache = seeded();
		expect(cache.readNearest(66_000)).toBe(cache.readMemory(66_000));
	});

	it("returns null before the first frame", () => {
		expect(seeded().readNearest(-1)).toBeNull();
	});

	it("never returns a frame older than the segment floor", () => {
		// The playhead is just past a cut ending at 66ms, so returning the 33ms frame would step back into cut content.
		const cache = seeded();
		expect(cache.readNearest(70_000, 66_000)).toBe(cache.readMemory(66_000));
		expect(cache.readNearest(50_000, 66_000)).toBeNull();
	});

	it("keeps the index correct after eviction", () => {
		const frameBytes = 64 * 64 * 4;
		const cache = new FrameCache({ memoryCapBytes: frameBytes * 2 });
		for (const ms of [0, 33, 66, 99]) cache.write(ms * 1000, fakeFrame(64, 64));
		// Oldest two evicted; a lookup below the survivors must not resurrect them.
		expect(cache.readNearest(10_000)).toBeNull();
		expect(cache.readNearest(99_000)).not.toBeNull();
	});

	it("drops the index when the scope changes", () => {
		const cache = seeded();
		cache.setScope("recording-b");
		expect(cache.readNearest(99_000)).toBeNull();
	});
});
