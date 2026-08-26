import {
	isUnsupportedCodec,
	isUnsupportedContainer,
	UNSUPPORTED_FORMATS,
	type UnsupportedFormat,
} from "@recast/media";
import { describe, expect, it } from "vitest";

/**
 * Pin the curated list of formats neither MediaBunny nor the legacy
 * webcodecs+mp4box pipeline can decode. This is the test that lives in
 * the consumer app so the desktop's `pnpm test:desktop` pipeline catches
 * drift in the package's exported list.
 */
describe("UNSUPPORTED_FORMATS (consumer-side pinning)", () => {
	it("exports a non-empty curated list", () => {
		expect(UNSUPPORTED_FORMATS.length).toBeGreaterThan(0);
	});

	it("every entry is well-formed", () => {
		const seenExt = new Set<string>();
		for (const f of UNSUPPORTED_FORMATS) {
			expect(f.container.length).toBeGreaterThan(0);
			expect(f.reason.length).toBeGreaterThan(0);
			for (const ext of f.container) {
				expect(ext).toBe(ext.toLowerCase());
				expect(ext.startsWith(".")).toBe(false);
				expect(seenExt.has(ext)).toBe(false);
				seenExt.add(ext);
			}
		}
	});

	it("contains the documented gap (AVI / FLV / WMV / RealVideo / 3GP)", () => {
		const containers = new Set<string>();
		for (const f of UNSUPPORTED_FORMATS as readonly UnsupportedFormat[]) {
			for (const ext of f.container) containers.add(ext);
		}
		for (const expected of ["avi", "flv", "wmv", "rm", "3gp"]) {
			expect(containers.has(expected)).toBe(true);
		}
	});
});

describe("isUnsupportedContainer / isUnsupportedCodec (consumer-side)", () => {
	it("classifies AVI / FLV / WMV as unsupported containers", () => {
		for (const ext of ["avi", "flv", "wmv", "rm", "3gp"]) {
			expect(isUnsupportedContainer(ext)).toBe(true);
			expect(isUnsupportedContainer(`.${ext.toUpperCase()}`)).toBe(true);
		}
	});

	it("classifies MP4 / MOV / WebM / MKV as supported containers", () => {
		for (const ext of ["mp4", "mov", "webm", "mkv", "wav", "mp3", "ogg", "flac"]) {
			expect(isUnsupportedContainer(ext)).toBe(false);
		}
	});

	it("classifies VC-1 / RealVideo as unsupported codecs", () => {
		expect(isUnsupportedCodec("vc-1")).toBe(true);
		expect(isUnsupportedCodec("VC-1")).toBe(true);
		expect(isUnsupportedCodec("realvideo")).toBe(true);
	});

	it("classifies AVC / HEVC / VP9 / AV1 / AAC / Opus / ALAC as supported codecs", () => {
		for (const c of ["avc", "h264", "hevc", "vp9", "av1", "aac", "opus", "alac", "flac", "mp3"]) {
			expect(isUnsupportedCodec(c)).toBe(false);
		}
	});
});
