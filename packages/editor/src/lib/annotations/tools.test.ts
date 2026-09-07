import { describe, expect, it, vi } from "vitest";

// @recast/icons re-exports a barrel that can't resolve under node, and the registry only stores components.
vi.mock("@recast/icons", () => ({
	__esModule: true,
	default: {},
	...Object.fromEntries(
		["MousePointer2", "Square", "Circle", "ArrowUpRight", "Type", "Droplets", "ImageIcon"].map(
			(name) => [name, () => null],
		),
	),
}));

const { ANNOTATION_TOOLS, IMAGE_TOOL, toolForHotkey } = await import("./tools");

describe("ANNOTATION_TOOLS", () => {
	it("leads with select, the way out of every drawing mode", () => {
		expect(ANNOTATION_TOOLS[0].id).toBe("select");
	});

	// Image is a one-shot file-picker insert, so its tile could never light up and placement has no branch for it.
	it("lists only modal tools", () => {
		expect(ANNOTATION_TOOLS.map((t) => t.id)).not.toContain("image");
	});

	it("assigns a unique hotkey per tool, image included", () => {
		const keys = [...ANNOTATION_TOOLS.map((t) => t.hotkey), IMAGE_TOOL.hotkey].map((k) =>
			k.toLowerCase(),
		);
		expect(new Set(keys).size).toBe(keys.length);
	});

	it("labels every tool", () => {
		for (const t of ANNOTATION_TOOLS) expect(t.label.trim()).not.toBe("");
	});
});

describe("toolForHotkey", () => {
	it("matches case-insensitively", () => {
		expect(toolForHotkey("r")?.id).toBe("rect");
		expect(toolForHotkey("R")?.id).toBe("rect");
	});

	it("returns null for a key that isn't a tool shortcut", () => {
		expect(toolForHotkey("q")).toBeNull();
	});

	// The panel checks the image key first, so this must not claim it or `I` would arm a mode instead of opening the picker.
	it("does not claim the image shortcut", () => {
		expect(toolForHotkey(IMAGE_TOOL.hotkey)).toBeNull();
	});
});
