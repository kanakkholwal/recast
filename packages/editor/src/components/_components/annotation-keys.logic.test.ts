import { describe, expect, it } from "vitest";
import { nudgeVectorPx, NUDGE_STEP_COARSE_PX, NUDGE_STEP_PX } from "./annotation-keys.logic";

function chord(
	key: string,
	mods: Partial<Record<"alt" | "ctrl" | "meta" | "shift", boolean>> = {},
) {
	return {
		key,
		altKey: mods.alt ?? false,
		ctrlKey: mods.ctrl ?? false,
		metaKey: mods.meta ?? false,
		shiftKey: mods.shift ?? false,
	};
}

describe("nudgeVectorPx", () => {
	// The player tooltips advertise bare arrows as frame-step. A selected shape
	// must not quietly take them over.
	it("ignores bare arrows so the transport keeps them", () => {
		for (const key of ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]) {
			expect(nudgeVectorPx(chord(key))).toBeNull();
		}
	});

	it("nudges on Alt+arrow", () => {
		expect(nudgeVectorPx(chord("ArrowLeft", { alt: true }))).toEqual({
			dx: -NUDGE_STEP_PX,
			dy: 0,
		});
		expect(nudgeVectorPx(chord("ArrowRight", { alt: true }))).toEqual({
			dx: NUDGE_STEP_PX,
			dy: 0,
		});
		expect(nudgeVectorPx(chord("ArrowUp", { alt: true }))).toEqual({ dx: 0, dy: -NUDGE_STEP_PX });
		expect(nudgeVectorPx(chord("ArrowDown", { alt: true }))).toEqual({ dx: 0, dy: NUDGE_STEP_PX });
	});

	it("takes the coarse step with Shift", () => {
		expect(nudgeVectorPx(chord("ArrowRight", { alt: true, shift: true }))?.dx).toBe(
			NUDGE_STEP_COARSE_PX,
		);
	});

	// Mod+arrow belongs to the OS and to word-wise navigation elsewhere.
	it("declines when Ctrl or Meta is also held", () => {
		expect(nudgeVectorPx(chord("ArrowLeft", { alt: true, ctrl: true }))).toBeNull();
		expect(nudgeVectorPx(chord("ArrowLeft", { alt: true, meta: true }))).toBeNull();
	});

	it("ignores non-arrow keys", () => {
		expect(nudgeVectorPx(chord("a", { alt: true }))).toBeNull();
	});
});
