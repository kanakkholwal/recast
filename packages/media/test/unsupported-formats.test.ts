import { describe, expect, it, vi } from "vitest";
import { MediabunnyVideoSource } from "../src/playback/source";
import {
	isUnsupportedCodec,
	isUnsupportedContainer,
	UNSUPPORTED_FORMATS,
} from "../src/cache/unsupported-formats";

/**
 * Pin the curated list of formats neither MediaBunny nor the legacy
 * webcodecs+mp4box pipeline can decode. Drift here would mean either:
 *   (a) MediaBunny added support and the list should shrink, or
 *   (b) someone removed a format and broke the desktop preview fallback.
 */
describe("UNSUPPORTED_FORMATS — gap MediaBunny + legacy cannot decode", () => {
	it("is non-empty (a fresh MediaBunny upgrade must update this list, not silently drop entries)", () => {
		expect(UNSUPPORTED_FORMATS.length).toBeGreaterThan(0);
	});

	it("every entry has non-empty container ext list and a non-empty reason", () => {
		for (const f of UNSUPPORTED_FORMATS) {
			expect(f.container.length).toBeGreaterThan(0);
			for (const ext of f.container) {
				expect(ext).toBe(ext.toLowerCase());
				expect(ext.startsWith(".")).toBe(false);
			}
			expect(f.reason.length).toBeGreaterThan(0);
		}
	});

	it("container extensions are unique across entries (no duplicates)", () => {
		const seen = new Set<string>();
		for (const f of UNSUPPORTED_FORMATS) {
			for (const ext of f.container) {
				expect(seen.has(ext)).toBe(false);
				seen.add(ext);
			}
		}
	});

	it("contains the formats we documented as unsupported", () => {
		const containers = new Set(UNSUPPORTED_FORMATS.flatMap((f) => f.container));
		// The user's documented gap: AVI, FLV, WMV/ASF, RealVideo, 3GP.
		for (const expected of ["avi", "flv", "wmv", "rm", "3gp"]) {
			expect(containers.has(expected)).toBe(true);
		}
	});
});

describe("isUnsupportedContainer / isUnsupportedCodec helpers", () => {
	it("isUnsupportedContainer recognises known extensions", () => {
		expect(isUnsupportedContainer("avi")).toBe(true);
		expect(isUnsupportedContainer(".AVI")).toBe(true);
		expect(isUnsupportedContainer("flv")).toBe(true);
		expect(isUnsupportedContainer("wmv")).toBe(true);
		expect(isUnsupportedContainer("3gp")).toBe(true);
	});

	it("isUnsupportedContainer returns false for supported extensions", () => {
		expect(isUnsupportedContainer("mp4")).toBe(false);
		expect(isUnsupportedContainer("mov")).toBe(false);
		expect(isUnsupportedContainer("webm")).toBe(false);
		expect(isUnsupportedContainer("mkv")).toBe(false);
	});

	it("isUnsupportedCodec recognises known unsupported codecs", () => {
		expect(isUnsupportedCodec("vc-1")).toBe(true);
		expect(isUnsupportedCodec("VC-1")).toBe(true);
		expect(isUnsupportedCodec("realvideo")).toBe(true);
	});

	it("isUnsupportedCodec returns false for supported codecs", () => {
		expect(isUnsupportedCodec("avc")).toBe(false);
		expect(isUnsupportedCodec("h264")).toBe(false);
		expect(isUnsupportedCodec("hevc")).toBe(false);
		expect(isUnsupportedCodec("vp9")).toBe(false);
		expect(isUnsupportedCodec("av1")).toBe(false);
		expect(isUnsupportedCodec("aac")).toBe(false);
		expect(isUnsupportedCodec("opus")).toBe(false);
		expect(isUnsupportedCodec("alac")).toBe(false);
	});
});
describe("MediabunnyVideoSource rejects known-bad containers up front", () => {
	it("throws unsupported for an .avi URL without spawning a worker", async () => {
		let spawned = 0;
		vi.stubGlobal("Worker", class {} as unknown as typeof Worker);
		vi.stubGlobal("VideoFrame", class {} as unknown as typeof VideoFrame);
		const createWorker = () => {
			spawned++;
			return {} as unknown as Worker;
		};
		try {
			await expect(
				MediabunnyVideoSource.create("asset://x/clip.avi", { createWorker }),
			).rejects.toMatchObject({
				code: "unsupported",
			});
			expect(spawned).toBe(0);
		} finally {
			vi.unstubAllGlobals();
		}
	});
});
