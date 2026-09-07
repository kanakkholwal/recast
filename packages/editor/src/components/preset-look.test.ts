import { describe, expect, it } from "vitest";
import { commitLook, type PresetLook, type PresetSource, previewLook } from "./preset-look";

const current: PresetLook = {
	bg: "wallpaper",
	value: "asset:wallpaper1",
	padding: 4,
	blur: 10,
	layout: "auto",
	aspect: "source",
	presetId: "focus",
};

const preset: PresetSource = {
	id: "ig-story",
	bg: "gradient",
	value: "linear-gradient(#000,#fff)",
	padding: 8,
	blur: 28,
	layout: "auto",
	aspect: "9:16",
};

describe("commitLook", () => {
	it("takes the preset's look and claims the preset id", () => {
		expect(commitLook(preset, current)).toEqual({
			bg: "gradient",
			value: "linear-gradient(#000,#fff)",
			padding: 8,
			blur: 28,
			layout: "auto",
			aspect: "9:16",
			presetId: "ig-story",
		});
	});

	it("falls back to the source canvas for an unmapped aspect", () => {
		expect(commitLook({ ...preset, aspect: "Source" }, current).aspect).toBe("source");
	});

	it("keeps the current background value when the preset carries none", () => {
		const { value, ...rest } = preset;
		expect(commitLook(rest, current).value).toBe("asset:wallpaper1");
	});
});

describe("previewLook", () => {
	// presetId feeds the picker's cursor, so changing it mid-preview regrouped the list and re-fired until the app died.
	it("never changes the applied preset id", () => {
		expect(previewLook(preset, current).presetId).toBe("focus");
		expect(previewLook(preset, { ...current, presetId: null }).presetId).toBeNull();
	});

	it("otherwise matches the committed look exactly", () => {
		const { presetId: _p, ...preview } = previewLook(preset, current);
		const { presetId: _c, ...commit } = commitLook(preset, current);
		expect(preview).toEqual(commit);
	});
});
