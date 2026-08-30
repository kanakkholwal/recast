import { describe, expect, it } from "vitest";
import {
	activeWordIndex,
	chunkWords,
	isStaticAnimation,
	resolveCaptionAnimation,
} from "./chunking";
import { karaokeCentiseconds, spokenWordCount } from "./highlight";
import { breakIntoLines } from "./linebreak";
import { pillBox } from "./geometry";
import { parseKaraokeCue, parseVttTime } from "./vtt";
import { CAPTION_PRESETS, DEFAULT_CAPTION_STYLE } from "./presets";
import type { CaptionAnimation, TranscriptWord } from "./types";

const words = (spec: [number, number, string][]): TranscriptWord[] =>
	spec.map(([start, end, text]) => ({ start, end, text }));

describe("resolveCaptionAnimation back-compat", () => {
	it("resolves an absent animation to a static, non-highlighting line", () => {
		const a = resolveCaptionAnimation(undefined);
		expect(a.highlight).toBe("none");
		expect(isStaticAnimation(a)).toBe(true);
	});

	it("resolves a pre-highlight animation to 'active', preserving old behaviour", () => {
		// A project saved before `highlight` existed: field is undefined.
		const legacy = {
			chunk: "line",
			chunkSize: 3,
			emphasis: "color",
			emphasisColor: "#facc15",
			entrance: "none",
			entranceMs: 220,
			holdGaps: true,
		} as CaptionAnimation;
		expect(resolveCaptionAnimation(legacy).highlight).toBe("active");
	});

	it("keeps an explicit progressive highlight", () => {
		const modern = { ...CAPTION_PRESETS[0].style.animation } as CaptionAnimation;
		expect(resolveCaptionAnimation(modern).highlight).toBe("progressive");
	});
});

describe("spokenWordCount", () => {
	const w = words([
		[0, 0.5, "a"],
		[0.5, 1, "b"],
		[1, 1.5, "c"],
	]);
	it("lights a word exactly at its start", () => {
		expect(spokenWordCount(w, 0.5)).toBe(2);
	});
	it("does not un-count an earlier word during a later gap", () => {
		const g = words([
			[0, 0.3, "a"],
			[1, 1.3, "b"],
		]);
		expect(spokenWordCount(g, 0.6)).toBe(1); // in the gap after "a"
	});
	it("is 0 before the first word and full after the last", () => {
		expect(spokenWordCount(w, -1)).toBe(0);
		expect(spokenWordCount(w, 99)).toBe(3);
	});
});

describe("karaokeCentiseconds", () => {
	it("sums to the rounded chunk span with no accumulated drift", () => {
		// 3 words, 0.333s each: independent rounding would drift, cumulative won't.
		const w = words([
			[0, 0.333, "a"],
			[0.333, 0.666, "b"],
			[0.666, 1.0, "c"],
		]);
		const cs = karaokeCentiseconds(w, 0);
		expect(cs.reduce((a, b) => a + b, 0)).toBe(100); // 1.00s total, exactly
	});
});

describe("breakIntoLines", () => {
	it("never splits inside a word", () => {
		const w = words([[0, 1, "supercalifragilistic"]]);
		expect(breakIntoLines(w, 5, 2)).toEqual([[0]]);
	});
	it("is idempotent on already-fitting input", () => {
		const w = words([
			[0, 1, "hi"],
			[1, 2, "yo"],
		]);
		expect(breakIntoLines(w, 42, 2)).toEqual([[0, 1]]);
	});
});

describe("pillBox", () => {
	it("clamps radius to half the pill height", () => {
		const box = pillBox({ ...DEFAULT_CAPTION_STYLE, boxRadiusEm: 99 }, 40, 300, 1);
		expect(box.radius).toBe(box.height / 2);
	});
	it("advance-derived width covers text plus both paddings", () => {
		const box = pillBox({ ...DEFAULT_CAPTION_STYLE, boxPaddingXEm: 0.5 }, 40, 300, 1);
		expect(box.width).toBe(300 + 2 * 0.5 * 40);
	});
});

describe("VTT karaoke parsing", () => {
	it("parses word timings out of a cue body", () => {
		// Literal, not round-tripped: Rust writes the VTT, so this pins the parser against the real wire format.
		const body = "<00:00:04.120>but <00:00:04.380>it's <00:00:04.600>a";
		const parsed = parseKaraokeCue(body, 4.12, 5.0);
		expect(parsed.map((p) => p.text)).toEqual(["but", "it's", "a"]);
		expect(parsed[1].start).toBeCloseTo(4.38, 3);
		expect(parsed[2].end).toBeCloseTo(5.0, 3);
	});

	it("parses a tagless cue as a single text run (older recasts)", () => {
		const parsed = parseKaraokeCue("plain sentence here", 1, 2);
		expect(parsed).toHaveLength(1);
		expect(parsed[0]).toMatchObject({ start: 1, end: 2, text: "plain sentence here" });
	});

	it("parses every stamp shape a cue can carry", () => {
		expect(parseVttTime("01:02:03.456")).toBeCloseTo(3723.456, 3);
		expect(parseVttTime("01:02.500")).toBeCloseTo(62.5, 3);
		expect(parseVttTime("12.340")).toBeCloseTo(12.34, 3);
	});
});

describe("activeWordIndex holdGaps", () => {
	const w = words([
		[0, 0.3, "a"],
		[1, 1.3, "b"],
	]);
	it("holds the last started word through a gap when holdGaps", () => {
		expect(activeWordIndex(w, 0.6, true)).toBe(0);
	});
	it("clears in a gap when not holding", () => {
		expect(activeWordIndex(w, 0.6, false)).toBe(-1);
	});
});

describe("presets", () => {
	it("ship Loom as the default first preset", () => {
		expect(CAPTION_PRESETS[0].id).toBe("loom");
		expect(DEFAULT_CAPTION_STYLE.animation?.highlight).toBe("progressive");
		expect(DEFAULT_CAPTION_STYLE.enabled).toBe(true);
	});
	it("give every preset the new required style fields", () => {
		for (const p of CAPTION_PRESETS) {
			expect(typeof p.style.mutedColor).toBe("string");
			expect(typeof p.style.boxRadiusEm).toBe("number");
			expect(typeof p.style.lineHeight).toBe("number");
			expect(typeof p.style.maxCharsPerLine).toBe("number");
			expect(p.style.animation?.highlight).toBeDefined();
		}
	});
});
