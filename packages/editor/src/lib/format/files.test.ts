import { describe, expect, it } from "vitest";
import { formatShortDate, formatSize, getExtension, relativeDate } from "./files";

describe("formatSize", () => {
	it("renders bytes below 1 KiB verbatim", () => {
		expect(formatSize(0)).toBe("0 B");
		expect(formatSize(512)).toBe("512 B");
		expect(formatSize(1023)).toBe("1023 B");
	});
	it("renders KiB and MiB with one decimal", () => {
		expect(formatSize(1024)).toBe("1.0 KB");
		expect(formatSize(1536)).toBe("1.5 KB");
		expect(formatSize(1048576)).toBe("1.0 MB");
		expect(formatSize(5 * 1048576)).toBe("5.0 MB");
	});
});

describe("getExtension", () => {
	it("upper-cases the extension after the last dot", () => {
		expect(getExtension("clip.mp4")).toBe("MP4");
		expect(getExtension("archive.tar.gz")).toBe("GZ");
	});
	it("falls back to FILE when there's no extension", () => {
		expect(getExtension("README")).toBe("FILE");
	});
});

describe("relativeDate", () => {
	// 2024-01-01 in epoch seconds, with a fixed `now` reference so the thresholds are deterministic.
	const base = Date.UTC(2024, 0, 1) / 1000;
	const now = (base + 0) * 1000;

	it("returns 'just now' under a minute", () => {
		expect(relativeDate(base - 30, { now })).toBe("just now");
	});
	it("returns minute / hour / day buckets", () => {
		expect(relativeDate(base - 120, { now })).toBe("2m ago");
		expect(relativeDate(base - 3 * 3600, { now })).toBe("3h ago");
		expect(relativeDate(base - 2 * 86400, { now })).toBe("2d ago");
	});
	it("falls back to an absolute date past a week", () => {
		const old = base - 30 * 86400;
		expect(relativeDate(old, { now })).toBe(formatShortDate(old));
	});
});
