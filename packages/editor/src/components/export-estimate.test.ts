import { describe, expect, it } from "vitest";
import { estimateExportBytes, formatByteRange, outputResolution } from "./export-estimate";

describe("outputResolution", () => {
	it("is the source itself when the preset has no bound", () => {
		expect(outputResolution(3840, 2160, "source")).toEqual({ width: 3840, height: 2160 });
	});

	it("never upscales a source smaller than the preset", () => {
		expect(outputResolution(1280, 720, "4k")).toEqual({ width: 1280, height: 720 });
	});

	// The filter decreases to fit, so a portrait clip at HD is 608x1080, not 1080p wide.
	it("fits portrait sources inside the bound without distorting them", () => {
		expect(outputResolution(1080, 1920, "hd")).toEqual({ width: 608, height: 1080 });
	});

	it("fits landscape sources to the width bound", () => {
		expect(outputResolution(2560, 1440, "hd")).toEqual({ width: 1920, height: 1080 });
	});

	it("rounds down to even dimensions, which yuv420p requires", () => {
		const r = outputResolution(1599, 962, "small");
		if (!r) throw new Error("expected a resolution");
		expect(r.width % 2).toBe(0);
		expect(r.height % 2).toBe(0);
	});

	it("is safe on unknown source dimensions", () => {
		expect(outputResolution(0, 0, "hd")).toBeNull();
	});
});

describe("estimateExportBytes", () => {
	it("scales with duration", () => {
		const short = estimateExportBytes({
			format: "mp4",
			quality: "hd",
			speed: "balanced",
			seconds: 10,
			width: 1920,
			height: 1080,
			fps: 30,
		});
		const long = estimateExportBytes({
			format: "mp4",
			quality: "hd",
			speed: "balanced",
			seconds: 20,
			width: 1920,
			height: 1080,
			fps: 30,
		});
		if (!short || !long) throw new Error("expected estimates");
		expect(long.low).toBeGreaterThan(short.low * 1.8);
	});

	it("puts a higher-quality preset above a smaller one", () => {
		const base = {
			format: "mp4" as const,
			speed: "balanced" as const,
			seconds: 30,
			width: 1920,
			height: 1080,
			fps: 30,
		};
		const small = estimateExportBytes({ ...base, quality: "small" });
		const uhd = estimateExportBytes({ ...base, quality: "4k" });
		if (!small || !uhd) throw new Error("expected estimates");
		expect(uhd.low).toBeGreaterThan(small.low);
	});

	it("returns a range, not a single number", () => {
		const e = estimateExportBytes({
			format: "mp4",
			quality: "hd",
			speed: "balanced",
			seconds: 30,
			width: 1920,
			height: 1080,
			fps: 30,
		});
		if (!e) throw new Error("expected an estimate");
		expect(e.high).toBeGreaterThan(e.low);
	});

	it("is null when the duration or resolution is unknown", () => {
		const args = {
			format: "mp4" as const,
			quality: "hd" as const,
			speed: "balanced" as const,
			width: 1920,
			height: 1080,
			fps: 30,
		};
		expect(estimateExportBytes({ ...args, seconds: 0 })).toBeNull();
		expect(estimateExportBytes({ ...args, seconds: 10, width: 0 })).toBeNull();
	});
});

describe("formatByteRange", () => {
	it("collapses to one figure when the range rounds the same", () => {
		expect(formatByteRange({ low: 5_100_000, high: 5_200_000 })).toBe("~5 MB");
	});

	it("shows a span when the ends differ", () => {
		expect(formatByteRange({ low: 4_000_000, high: 9_000_000 })).toBe("4–9 MB");
	});

	it("uses GB past a thousand megabytes", () => {
		expect(formatByteRange({ low: 2_000_000_000, high: 2_000_000_000 })).toContain("GB");
	});
});
