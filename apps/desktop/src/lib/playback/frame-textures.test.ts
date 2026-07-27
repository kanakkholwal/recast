import { describe, expect, it } from "vitest";
import { pickSlot, type RingSlot } from "./frame-textures";

const slots = (...ts: number[]): RingSlot[] => ts.map((tsUs) => ({ tsUs }));

describe("pickSlot", () => {
	it("returns the newest frame at or before the playhead", () => {
		// A ring wraps, so slot order is not timestamp order.
		const ring = slots(3_000, 4_000, 1_000, 2_000);
		expect(pickSlot(ring, 2_500, 0)).toBe(3); // 2_000
		expect(pickSlot(ring, 4_000, 0)).toBe(1); // 4_000
	});

	it("never returns a frame ahead of the playhead", () => {
		expect(pickSlot(slots(5_000, 6_000), 1_000, 0)).toBe(-1);
	});

	it("never returns a frame before the segment floor", () => {
		// The floor is the start of the current kept segment; anything earlier
		// is inside a removed cut and would step the picture backwards.
		const ring = slots(1_000, 9_000);
		expect(pickSlot(ring, 10_000, 5_000)).toBe(1);
		expect(pickSlot(ring, 4_000, 5_000)).toBe(-1);
	});

	it("ignores empty slots", () => {
		expect(pickSlot(slots(-1, -1, -1), 10_000, 0)).toBe(-1);
		expect(pickSlot(slots(-1, 2_000, -1), 10_000, 0)).toBe(1);
	});

	it("accepts a frame exactly on the playhead and on the floor", () => {
		expect(pickSlot(slots(2_000), 2_000, 0)).toBe(0);
		expect(pickSlot(slots(2_000), 5_000, 2_000)).toBe(0);
	});

	it("handles an empty ring", () => {
		expect(pickSlot([], 1_000, 0)).toBe(-1);
	});
});
