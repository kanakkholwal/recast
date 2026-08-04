import { CAPTION_PRESETS, type CaptionStyle } from "@recast/captions";
import { describe, expect, it } from "vitest";
import type { CaptionModelInfo, TranscriptSegment } from "$lib/ipc-types";
import type { CaptionPresetValue } from "$lib/registry/types";
import {
	captionStyleMatchesPreset,
	downloadProgressPct,
	elapsedLabel,
	filterSegments,
	groupModelsByFamily,
	langLabel,
	pickDefaultModelId,
} from "./captions-panel.logic";

/** A fully-populated model row; override only what a case cares about. */
function model(over: Partial<CaptionModelInfo> = {}): CaptionModelInfo {
	return {
		id: "m",
		displayName: "Model",
		engine: "ggml",
		runtime: "ggml",
		source: "builtin",
		family: "Parakeet",
		languages: ["en"],
		approxSizeBytes: null,
		isDefault: false,
		installed: true,
		downloadable: true,
		requiresGpu: false,
		prefersGpu: false,
		minRamBytes: null,
		runnable: true,
		runtimeAvailable: true,
		warning: null,
		capabilities: { streaming: false, translate: false, langDetect: false, timestamps: "token" },
		languageCount: null,
		speedScore: null,
		accuracyScore: null,
		recommended: false,
		...over,
	};
}

function seg(id: string, start: number, text: string): TranscriptSegment {
	return { id, start, end: start + 3, text, words: [] };
}

describe("pickDefaultModelId", () => {
	it("returns null for an empty list", () => {
		expect(pickDefaultModelId([])).toBeNull();
	});

	it("prefers an installed, runnable, runtime-available default", () => {
		const models = [model({ id: "a" }), model({ id: "b", isDefault: true })];
		expect(pickDefaultModelId(models)).toBe("b");
	});

	it("skips a model whose runtime is unavailable even if it is the default", () => {
		// A default model whose runtime isn't usable here (e.g. a remote endpoint
		// with no key) must not be auto-selected over a usable on-device model.
		const models = [
			model({ id: "remote", isDefault: true, runtimeAvailable: false }),
			model({ id: "parakeet" }),
		];
		expect(pickDefaultModelId(models)).toBe("parakeet");
	});

	it("skips models that are not installed or not runnable", () => {
		const models = [
			model({ id: "notinstalled", installed: false }),
			model({ id: "notrunnable", runnable: false }),
			model({ id: "usable" }),
		];
		expect(pickDefaultModelId(models)).toBe("usable");
	});

	it("falls back to a flagged default when nothing is usable", () => {
		const models = [
			model({ id: "x", installed: false }),
			model({ id: "y", isDefault: true, runtimeAvailable: false }),
		];
		// No usable model, so the flagged default wins the fallback.
		expect(pickDefaultModelId(models)).toBe("y");
	});
});

describe("groupModelsByFamily", () => {
	it("groups by family, preserving first-seen order", () => {
		const models = [
			model({ id: "p1", family: "Parakeet" }),
			model({ id: "w1", family: "Whisper" }),
			model({ id: "p2", family: "Parakeet" }),
		];
		const groups = groupModelsByFamily(models);
		expect(groups.map((g) => g.name)).toEqual(["Parakeet", "Whisper"]);
		expect(groups[0].models.map((m) => m.id)).toEqual(["p1", "p2"]);
		expect(groups[1].models.map((m) => m.id)).toEqual(["w1"]);
	});
});

describe("langLabel", () => {
	it("labels multilingual models", () => {
		expect(langLabel(model({ languages: ["multi"] }))).toBe("Multilingual");
	});
	it("upper-cases explicit language codes", () => {
		expect(langLabel(model({ languages: ["en", "hi"] }))).toBe("EN, HI");
	});
	it("prefers the registry's language count over the vague 'multi' hint", () => {
		expect(langLabel(model({ languages: ["multi"], languageCount: 28 }))).toBe("28 languages");
	});
	it("keeps the code list for a single-language model", () => {
		expect(langLabel(model({ languages: ["en"], languageCount: 1 }))).toBe("EN");
	});
});

describe("downloadProgressPct", () => {
	it("clamps and rounds, and is 0 when total is unknown", () => {
		expect(downloadProgressPct(50, 100)).toBe(50);
		expect(downloadProgressPct(200, 100)).toBe(100);
		expect(downloadProgressPct(10, 0)).toBe(0);
	});
});

describe("captionStyleMatchesPreset", () => {
	const loom = CAPTION_PRESETS[0];
	const cs = (): CaptionStyle => ({ enabled: true, ...loom.style });
	const val = loom.style as CaptionPresetValue;

	it("matches a style built straight from the preset", () => {
		expect(captionStyleMatchesPreset(cs(), val)).toBe(true);
	});

	it("does NOT match when only the highlight mode differs", () => {
		const tweaked: CaptionStyle = {
			...cs(),
			animation: { ...loom.style.animation!, highlight: "active" },
		};
		expect(captionStyleMatchesPreset(tweaked, val)).toBe(false);
	});

	it("does NOT match when only a pill field differs", () => {
		expect(captionStyleMatchesPreset({ ...cs(), boxRadiusEm: 0.1 }, val)).toBe(false);
		expect(captionStyleMatchesPreset({ ...cs(), mutedColor: "#123456" }, val)).toBe(false);
	});

	it("treats an absent animation as its resolved default on both sides", () => {
		// A preset with no animation vs a style with no animation still matches
		// (both resolve identically), so the readout does not falsely say Custom.
		const plainVal = { ...val, animation: undefined } as CaptionPresetValue;
		const plainCs: CaptionStyle = { ...cs(), animation: undefined };
		expect(captionStyleMatchesPreset(plainCs, plainVal)).toBe(true);
	});
});

describe("filterSegments", () => {
	const segs = [
		seg("a", 0, "Welcome to the demo"),
		seg("b", 4, "Let's open the editor"),
		seg("c", 9, "OPEN the export dialog"),
	];

	it("returns the list untouched for an empty or whitespace query", () => {
		expect(filterSegments(segs, "")).toBe(segs);
		expect(filterSegments(segs, "   ")).toBe(segs);
	});

	it("matches case-insensitively", () => {
		expect(filterSegments(segs, "open").map((s) => s.id)).toEqual(["b", "c"]);
	});

	it("matches on the whole phrase, not on each word separately", () => {
		// "the demo" must not also pull in every line containing "the".
		expect(filterSegments(segs, "the demo").map((s) => s.id)).toEqual(["a"]);
	});

	it("returns nothing when no line matches", () => {
		expect(filterSegments(segs, "zzz")).toEqual([]);
	});
});

describe("elapsedLabel", () => {
	it("counts seconds under a minute", () => {
		expect(elapsedLabel(0)).toBe("0s");
		expect(elapsedLabel(9_400)).toBe("9s");
	});

	it("switches to m:ss at a minute", () => {
		expect(elapsedLabel(60_000)).toBe("1:00");
		expect(elapsedLabel(125_000)).toBe("2:05");
	});

	// A negative delta means the clock moved backwards mid-run; showing "-3s"
	// would look like a bug in the progress readout.
	it("never renders a negative time", () => {
		expect(elapsedLabel(-3_000)).toBe("0s");
	});
});
