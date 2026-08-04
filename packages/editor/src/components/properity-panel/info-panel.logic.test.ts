import { describe, expect, it } from "vitest";
import type { Annotation } from "$lib/stores/editor-store.svelte";
import { basename, countByKind, formatRelative } from "./info-panel.logic";

describe("formatRelative", () => {
	const now = 1_000_000_000_000;
	it("returns Never for null", () => {
		expect(formatRelative(null, now)).toBe("Never");
	});
	it("buckets past times with ' ago'", () => {
		expect(formatRelative(now - 2000, now)).toBe("just now");
		expect(formatRelative(now - 10_000, now)).toBe("10s ago");
		expect(formatRelative(now - 5 * 60_000, now)).toBe("5 min ago");
		expect(formatRelative(now - 3 * 3_600_000, now)).toBe("3 hr ago");
		expect(formatRelative(now - 2 * 86_400_000, now)).toBe("2 days ago");
	});
	it("prefixes future times with 'in '", () => {
		expect(formatRelative(now + 5 * 60_000, now)).toBe("in 5 min");
	});
	it("singularises one day", () => {
		expect(formatRelative(now - 86_400_000, now)).toBe("1 day ago");
	});
});

describe("basename", () => {
	it("takes the last segment of either separator", () => {
		expect(basename("C:\\a\\b\\clip.mp4")).toBe("clip.mp4");
		expect(basename("/home/u/clip.mp4")).toBe("clip.mp4");
		expect(basename("")).toBe("—");
	});
});

describe("countByKind", () => {
	it("seeds all kinds to 0 and counts present ones", () => {
		const anns = [
			{ kind: { kind: "rect" } },
			{ kind: { kind: "rect" } },
			{ kind: { kind: "arrow" } },
		] as unknown as Annotation[];
		expect(countByKind(anns)).toEqual({
			rect: 2,
			ellipse: 0,
			arrow: 1,
			text: 0,
			image: 0,
		});
	});
});
