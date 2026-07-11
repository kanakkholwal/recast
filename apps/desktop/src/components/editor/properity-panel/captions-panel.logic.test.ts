import { describe, expect, it } from "vitest";
import type { CaptionModelInfo } from "$lib/ipc";
import {
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
		engine: "parakeet",
		runtime: "onnx",
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
		// A whisperCpp default is installed but its runtime isn't built yet, so it
		// must not be auto-selected over a usable onnx model.
		const models = [
			model({ id: "whisper", isDefault: true, runtimeAvailable: false }),
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
			model({ id: "c1", family: "Canary" }),
			model({ id: "p2", family: "Parakeet" }),
		];
		const groups = groupModelsByFamily(models);
		expect(groups.map((g) => g.name)).toEqual(["Parakeet", "Canary"]);
		expect(groups[0].models.map((m) => m.id)).toEqual(["p1", "p2"]);
		expect(groups[1].models.map((m) => m.id)).toEqual(["c1"]);
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
