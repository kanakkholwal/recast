import { describe, expect, it } from "vitest";
import type { RecordingEntry } from "$lib/ipc";
import { greeting, mergeRecents } from "./home.logic";

const entry = (filename: string, created: number): RecordingEntry =>
	({ filename, path: filename, sizeBytes: 0, created }) as RecordingEntry;

describe("greeting", () => {
	it("splits the day into morning, afternoon, evening", () => {
		expect(greeting(new Date(2026, 0, 1, 8))).toBe("Good morning");
		expect(greeting(new Date(2026, 0, 1, 13))).toBe("Good afternoon");
		expect(greeting(new Date(2026, 0, 1, 20))).toBe("Good evening");
	});

	it("treats noon as afternoon and 6pm as evening", () => {
		expect(greeting(new Date(2026, 0, 1, 12))).toBe("Good afternoon");
		expect(greeting(new Date(2026, 0, 1, 18))).toBe("Good evening");
	});
});

describe("mergeRecents", () => {
	it("interleaves both kinds newest-first and tags each", () => {
		const recordings = [entry("rec-old", 100), entry("rec-new", 400)];
		const exports = [entry("exp-mid", 300), entry("exp-older", 200)];

		const merged = mergeRecents(recordings, exports, 10);

		expect(merged.map((m) => m.entry.filename)).toEqual([
			"rec-new",
			"exp-mid",
			"exp-older",
			"rec-old",
		]);
		expect(merged[0].kind).toBe("recording");
		expect(merged[1].kind).toBe("export");
	});

	it("caps the result at the limit", () => {
		const recordings = Array.from({ length: 5 }, (_, i) => entry(`r${i}`, i));
		expect(mergeRecents(recordings, [], 3)).toHaveLength(3);
	});

	it("returns an empty list when there is nothing", () => {
		expect(mergeRecents([], [], 8)).toEqual([]);
	});
});
