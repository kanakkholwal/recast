import { describe, expect, it } from "vitest";
import type { RecordingEntry } from "$lib/ipc";
import { classify, countByKind, filterMedia, sortMedia, toItems } from "./media.logic";

const entry = (filename: string, created = 0, sizeBytes = 0): RecordingEntry =>
	({ filename, path: filename, created, sizeBytes }) as RecordingEntry;

describe("classify", () => {
	it("maps extensions to media kinds", () => {
		expect(classify("clip.mp4")).toBe("video");
		expect(classify("Screen 2026.recast")).toBe("video");
		expect(classify("voice.wav")).toBe("audio");
		expect(classify("logo.png")).toBe("image");
		expect(classify("notes.txt")).toBe("other");
	});
});

describe("toItems + filter + count", () => {
	const items = toItems(
		[entry("a.recast"), entry("b.mp4")],
		[entry("c.mp4"), entry("d.png"), entry("e.wav")],
	);

	it("tags source and kind", () => {
		expect(items).toHaveLength(5);
		expect(items[0]).toMatchObject({ source: "recording", kind: "video" });
		expect(items[3]).toMatchObject({ source: "export", kind: "image" });
	});

	it("filters by tab and query", () => {
		expect(filterMedia(items, "video", "").map((m) => m.entry.filename)).toEqual([
			"a.recast",
			"b.mp4",
			"c.mp4",
		]);
		expect(filterMedia(items, "all", "d").map((m) => m.entry.filename)).toEqual(["d.png"]);
	});

	it("counts by kind", () => {
		expect(countByKind(items)).toEqual({ all: 5, video: 3, audio: 1, image: 1, other: 0 });
	});
});

describe("sortMedia", () => {
	const items = toItems(
		[entry("old.mp4", 100, 5)],
		[entry("new.mp4", 400, 1), entry("mid.mp4", 200, 9)],
	);

	it("orders newest-first, by name, or largest-first", () => {
		expect(sortMedia(items, "recent").map((m) => m.entry.filename)).toEqual([
			"new.mp4",
			"mid.mp4",
			"old.mp4",
		]);
		expect(sortMedia(items, "name").map((m) => m.entry.filename)).toEqual([
			"mid.mp4",
			"new.mp4",
			"old.mp4",
		]);
		expect(sortMedia(items, "size").map((m) => m.entry.filename)).toEqual([
			"mid.mp4",
			"old.mp4",
			"new.mp4",
		]);
	});
});
