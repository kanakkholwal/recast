import { describe, expect, it } from "vitest";
import { createRateTracker } from "./transfer-rate";

describe("createRateTracker", () => {
	it("has no estimate on the first sample", () => {
		const t = createRateTracker();
		expect(t.sample("a", 0, 0)).toBeUndefined();
	});

	it("estimates bytes/sec from the second sample onward", () => {
		const t = createRateTracker();
		t.sample("a", 0, 0);
		// 1 MB after 1s → 1 MB/s on the first real estimate.
		expect(t.sample("a", 1_000_000, 1000)).toBe(1_000_000);
	});

	it("ignores samples closer than the gate and keeps the last estimate", () => {
		const t = createRateTracker();
		t.sample("a", 0, 0);
		const r = t.sample("a", 1_000_000, 1000);
		// 100ms later (< 0.2s gate): no recompute, prior estimate stands.
		expect(t.sample("a", 1_050_000, 1100)).toBe(r);
	});

	it("smooths toward a new rate rather than jumping", () => {
		const t = createRateTracker();
		t.sample("a", 0, 0);
		t.sample("a", 1_000_000, 1000); // 1 MB/s
		// Next second sends 3 MB (3 MB/s instant); EMA = 0.6*1M + 0.4*3M = 1.8 MB/s.
		expect(t.sample("a", 4_000_000, 2000)).toBeCloseTo(1_800_000, 0);
	});

	it("tracks keys independently and clears them", () => {
		const t = createRateTracker();
		t.sample("a", 0, 0);
		t.sample("b", 0, 0);
		expect(t.sample("a", 500_000, 1000)).toBe(500_000);
		expect(t.sample("b", 2_000_000, 1000)).toBe(2_000_000);
		t.clear("a");
		expect(t.sample("a", 999, 1500)).toBeUndefined();
	});
});
