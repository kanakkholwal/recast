import { describe, expect, it } from "vitest";
import type { CaptionModelInfo } from "$lib/ipc-types";
import { CAPTION_PRESETS, type CaptionStyle } from "@recast/captions";
import type { CaptionPresetValue } from "$lib/registry/types";
import {
	captionStyleMatchesPreset,
	downloadProgressPct,
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
		...over,
	};
}

describe("pickDefaultModelId", () => {
	it("returns null for an empty list", () => {
		expect(pickDefaultModelId([])).toBeNull();
	});

	it("prefers an installed, runnable, runtime-available default", () => {
		const models = [
			model({ id: "a" }),
			model({ id: "b", isDefault: true }),
		];
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
