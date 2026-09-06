import { describe, expect, it } from "vitest";
import { fileRangeFor, keepInWindow } from "./audio-chunk-store";

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
		// Played to 25 min, then jumped back: the old one-sided evict kept everything ahead and stranded that window.
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

describe("fileRangeFor", () => {
	it("is the identity for a track that started with the video", () => {
		expect(fileRangeFor(4, 12, 0)).toEqual({ start: 4, end: 12 });
	});

	it("reads earlier in the file for a track that started late", () => {
		// Mic came up 0.5s after video frame 0, so timeline 4s is mic-file 3.5s.
		expect(fileRangeFor(4, 12, 0.5)).toEqual({ start: 3.5, end: 11.5 });
	});

	it("reads later in the file for a track that started early", () => {
		expect(fileRangeFor(4, 12, -0.5)).toEqual({ start: 4.5, end: 12.5 });
	});

	it("never asks for time before the file starts", () => {
		expect(fileRangeFor(0, 2, 1.5)).toEqual({ start: 0, end: 0.5 });
	});

	it("collapses rather than inverting when the whole range predates the track", () => {
		const range = fileRangeFor(0, 1, 5);
		expect(range.start).toBe(0);
		expect(range.end).toBeLessThanOrEqual(range.start);
	});

	it("ignores a non-finite offset instead of poisoning the range", () => {
		expect(fileRangeFor(4, 12, Number.NaN)).toEqual({ start: 4, end: 12 });
	});
});
