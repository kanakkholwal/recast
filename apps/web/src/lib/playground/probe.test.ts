import { describe, expect, it } from "vitest";
import { ACCEPTED_EXTENSIONS, isAcceptedFile, maxDurationSec } from "./probe";

const file = (name: string, type = "") => new File([new Uint8Array(4)], name, { type });

describe("isAcceptedFile", () => {
	it("accepts every advertised extension", () => {
		for (const ext of ACCEPTED_EXTENSIONS) expect(isAcceptedFile(file(`clip.${ext}`))).toBe(true);
	});

	it("accepts an extensionless file with a video MIME type", () => {
		expect(isAcceptedFile(file("recording", "video/mp4"))).toBe(true);
	});

	it("rejects a non-video", () => {
		expect(isAcceptedFile(file("notes.pdf", "application/pdf"))).toBe(false);
	});

	it("is case-insensitive about the extension", () => {
		expect(isAcceptedFile(file("CLIP.MP4"))).toBe(true);
	});
});

describe("maxDurationSec", () => {
	// A three-pane NLE on a phone is not the experience; cap hard and funnel.
	it("caps mobile tightly regardless of reported memory", () => {
		expect(maxDurationSec(32, true)).toBe(60);
	});

	it("scales with device memory on desktop", () => {
		expect(maxDurationSec(16, false)).toBeGreaterThan(maxDurationSec(8, false));
		expect(maxDurationSec(8, false)).toBeGreaterThan(maxDurationSec(4, false));
		expect(maxDurationSec(2, false)).toBeLessThan(maxDurationSec(4, false));
	});

	// navigator.deviceMemory is absent in Safari and Firefox; assume mid-range
	// rather than the floor, or those browsers get a needlessly tiny limit.
	it("assumes a mid-range device when memory is unreported", () => {
		expect(maxDurationSec(null, false)).toBe(maxDurationSec(4, false));
	});
});
