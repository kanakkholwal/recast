import { describe, expect, it } from "vitest";
import {
	MAX_OFFSET_MS,
	resolveTrackOffsets,
	trackTimeAt,
	ZERO_TRACK_OFFSETS,
} from "./track-offsets";

describe("resolveTrackOffsets", () => {
	it("treats a project with no measurement as aligned", () => {
		expect(resolveTrackOffsets(undefined)).toEqual(ZERO_TRACK_OFFSETS);
		expect(resolveTrackOffsets(null)).toEqual(ZERO_TRACK_OFFSETS);
		expect(resolveTrackOffsets({})).toEqual(ZERO_TRACK_OFFSETS);
	});

	it("keeps measured offsets of either sign", () => {
		expect(resolveTrackOffsets({ audioMs: 240, microphoneMs: -180, cameraMs: 60 })).toEqual({
			audioMs: 240,
			microphoneMs: -180,
			cameraMs: 60,
		});
	});

	it("drops a per-track measurement that is missing without losing the others", () => {
		expect(resolveTrackOffsets({ audioMs: 100, microphoneMs: null })).toEqual({
			audioMs: 100,
			microphoneMs: 0,
			cameraMs: 0,
		});
	});

	it("refuses an implausible measurement rather than wrecking the timeline", () => {
		expect(resolveTrackOffsets({ cameraMs: MAX_OFFSET_MS + 1 }).cameraMs).toBe(0);
		expect(resolveTrackOffsets({ cameraMs: -(MAX_OFFSET_MS + 1) }).cameraMs).toBe(0);
		expect(resolveTrackOffsets({ cameraMs: MAX_OFFSET_MS }).cameraMs).toBe(MAX_OFFSET_MS);
	});

	it("survives non-finite values", () => {
		expect(resolveTrackOffsets({ audioMs: Number.NaN, cameraMs: Infinity })).toEqual(
			ZERO_TRACK_OFFSETS,
		);
	});

	it("returns a fresh object so callers cannot mutate the shared zero", () => {
		const a = resolveTrackOffsets(undefined);
		a.audioMs = 999;
		expect(resolveTrackOffsets(undefined).audioMs).toBe(0);
		expect(ZERO_TRACK_OFFSETS.audioMs).toBe(0);
	});
});

describe("trackTimeAt", () => {
	it("is the identity for an aligned track", () => {
		expect(trackTimeAt(5, 0)).toBe(5);
	});

	it("reads earlier in a track that started late", () => {
		expect(trackTimeAt(5, 500)).toBe(4.5);
	});

	it("reads later in a track that started early", () => {
		expect(trackTimeAt(5, -500)).toBe(5.5);
	});

	it("clamps to the track's first sample before it existed", () => {
		expect(trackTimeAt(0.2, 900)).toBe(0);
	});
});
