import { describe, expect, it } from "vitest";
import {
	CLIP_BASE,
	CLIP_FOCUS,
	CLIP_HOVER,
	CLIP_LABEL,
	CLIP_META,
	CLIP_SELECTED,
	clipSurface,
	type LaneTone,
} from "./timeline-clip.styles";

const TONES: LaneTone[] = ["zoom", "markup", "music", "audio", "cut"];

describe("clipSurface", () => {
	it("covers every lane", () => {
		for (const tone of TONES) {
			expect(clipSurface(tone), tone).toBeTruthy();
		}
	});

	// Tailwind scans source text, so a name built at runtime is never generated and the class silently does nothing.
	it("names classes literally, never composed from the tone", () => {
		for (const tone of TONES) {
			const surface = clipSurface(tone);
			for (const value of Object.values(surface)) {
				expect(value, `${tone}: ${value}`).not.toContain("${");
				expect(value.trim().length, `${tone}: ${value}`).toBeGreaterThan(0);
			}
		}
	});

	it("gives each lane its own fill", () => {
		const fills = TONES.map((t) => clipSurface(t).fill);
		expect(new Set(fills).size).toBe(TONES.length);
	});
});

describe("shared clip classes", () => {
	// The bug: CLIP_BASE opened with `relative`, which Tailwind emits after `absolute`, so `inset-0` stopped applying and clips shrank to their label.
	const POSITION = /(?:^|\s)(?:static|fixed|absolute|relative|sticky)(?:\s|$)/;

	it("leaves positioning to the consumer", () => {
		expect(CLIP_BASE).not.toMatch(POSITION);
	});

	it("does not set a size either — rows own that", () => {
		expect(CLIP_BASE).not.toMatch(/(?:^|\s)(?:h-|w-|size-)/);
	});

	it("keeps the state classes free of layout", () => {
		for (const value of [CLIP_HOVER, CLIP_SELECTED, CLIP_FOCUS]) {
			expect(value, value).not.toMatch(POSITION);
		}
	});

	it("marks label text as non-interactive so it can't eat a drag", () => {
		expect(CLIP_LABEL).toContain("pointer-events-none");
		expect(CLIP_META).toContain("pointer-events-none");
	});
});
