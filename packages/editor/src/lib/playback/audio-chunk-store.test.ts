import { describe, expect, it } from "vitest";
import { keepInWindow } from "./audio-chunk-store";

/** 4-second chunks on a grid, like the decoder produces. */
const chunks = (count: number, size = 4) =>
	Array.from({ length: count }, (_, i) => ({ startSec: i * size, durationSec: size }));

const starts = (rows: ReadonlyArray<{ startSec: number }>) => rows.map((r) => r.startSec);

describe("keepInWindow", () => {
	it("keeps only chunks overlapping the window", () => {
		expect(starts(keepInWindow(chunks(10), 12, 24))).toEqual([12, 16, 20]);
	});

	it("keeps a chunk straddling either edge", () => {
		// [8,12) straddles the start, [24,28) straddles the end.
		expect(starts(keepInWindow(chunks(10), 10, 26))).toEqual([8, 12, 16, 20, 24]);
	});

	it("drops chunks ahead of the window — the backward-seek leak", () => {
		// Played to 25 min, then jumped back to the start. The old one-sided evict
		// kept everything ahead of the marker, stranding that window forever.
		const resident = [
			{ startSec: 1500, durationSec: 4 },
			{ startSec: 1504, durationSec: 4 },
			{ startSec: 0, durationSec: 4 },
		];
		expect(starts(keepInWindow(resident, 0, 16))).toEqual([0]);
	});

	it("keeps everything when the range is inverted", () => {
		// Degrades to the old retain-all rather than evicting audio about to play.
		const resident = chunks(4);
		expect(keepInWindow(resident, 20, 4)).toHaveLength(4);
	});

	it("bounds residency no matter how far playback has advanced", () => {
		let resident = chunks(500); // 2000s decoded
		for (let heard = 0; heard < 2000; heard += 4) {
			resident = keepInWindow(resident, heard - 4, heard + 16);
			expect(resident.length).toBeLessThanOrEqual(6);
		}
	});

	it("returns an empty set when nothing overlaps", () => {
		expect(keepInWindow(chunks(4), 100, 200)).toEqual([]);
	});
});
