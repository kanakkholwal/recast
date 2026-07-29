import { describe, expect, it } from "vitest";
import { QUALITY_HIGH, QUALITY_LOW, QUALITY_MEDIUM, QUALITY_VERY_HIGH } from "@recast/media/mediabunny";
import { exportFrameCount, exportFrameTime, videoEncodingConfigFor } from "./browser-export-plan";

describe("exportFrameCount", () => {
	it("rounds duration × fps and clamps to at least one frame", () => {
		expect(exportFrameCount(30, 2)).toBe(60);
		expect(exportFrameCount(60, 10)).toBe(600);
		expect(exportFrameCount(30, 0.01)).toBe(1);
	});

	it("returns 0 for degenerate inputs", () => {
		expect(exportFrameCount(0, 10)).toBe(0);
		expect(exportFrameCount(30, 0)).toBe(0);
		expect(exportFrameCount(-30, 10)).toBe(0);
	});
});

describe("exportFrameTime", () => {
	it("maps frame index to output seconds at fps", () => {
		expect(exportFrameTime(0, 30)).toBe(0);
		expect(exportFrameTime(30, 30)).toBe(1);
		expect(exportFrameTime(15, 30)).toBe(0.5);
	});
});

describe("videoEncodingConfigFor", () => {
	it("maps quality tiers to MediaBunny Quality presets, always H.264", () => {
		expect(videoEncodingConfigFor("low")).toMatchObject({ codec: "avc", bitrate: QUALITY_LOW });
		expect(videoEncodingConfigFor("medium").bitrate).toBe(QUALITY_MEDIUM);
		expect(videoEncodingConfigFor("high").bitrate).toBe(QUALITY_HIGH);
		expect(videoEncodingConfigFor("max").bitrate).toBe(QUALITY_VERY_HIGH);
	});

	it("carries the key-frame interval", () => {
		expect(videoEncodingConfigFor("high", 4).keyFrameInterval).toBe(4);
		expect(videoEncodingConfigFor("high").keyFrameInterval).toBe(2);
	});
});
