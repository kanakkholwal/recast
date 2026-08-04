import { describe, expect, it, vi } from "vitest";
import { DecoderBudget, secondaryPaused } from "./decoder-budget";

describe("secondaryPaused", () => {
	it("pauses every secondary while the preview is busy", () => {
		expect(secondaryPaused(true, 0, 1)).toBe(true);
		expect(secondaryPaused(true, 5, 10)).toBe(true);
	});

	it("admits secondaries under the cap when the preview is idle", () => {
		expect(secondaryPaused(false, 0, 1)).toBe(false);
		expect(secondaryPaused(false, 1, 2)).toBe(false);
	});

	it("pauses secondaries at or beyond the cap when idle", () => {
		expect(secondaryPaused(false, 1, 1)).toBe(true);
		expect(secondaryPaused(false, 2, 2)).toBe(true);
	});

	it("clamps the cap to at least one", () => {
		expect(secondaryPaused(false, 0, 0)).toBe(false);
		expect(secondaryPaused(false, 1, 0)).toBe(true);
	});
});

describe("DecoderBudget", () => {
	it("starts a registered secondary resumed when the preview is idle", () => {
		const onPause = vi.fn();
		new DecoderBudget(1).registerSecondary({ onPause });
		expect(onPause).toHaveBeenLastCalledWith(false);
	});

	it("pauses the secondary when the preview goes busy and resumes when idle", () => {
		const budget = new DecoderBudget(1);
		const onPause = vi.fn();
		budget.registerSecondary({ onPause });
		budget.setPreviewBusy(true);
		expect(onPause).toHaveBeenLastCalledWith(true);
		budget.setPreviewBusy(false);
		expect(onPause).toHaveBeenLastCalledWith(false);
	});

	it("only re-notifies on a real busy-state change", () => {
		const budget = new DecoderBudget(1);
		const onPause = vi.fn();
		budget.registerSecondary({ onPause }); // initial: false
		budget.setPreviewBusy(false); // no change
		expect(onPause).toHaveBeenCalledTimes(1);
	});

	it("pauses a second concurrent secondary beyond the cap of one", () => {
		const budget = new DecoderBudget(1);
		const first = vi.fn();
		const second = vi.fn();
		budget.registerSecondary({ onPause: first });
		budget.registerSecondary({ onPause: second });
		expect(first).toHaveBeenLastCalledWith(false); // index 0, under cap
		expect(second).toHaveBeenLastCalledWith(true); // index 1, at cap
	});

	it("re-evaluates remaining leases after one unregisters", () => {
		const budget = new DecoderBudget(1);
		const first = vi.fn();
		const second = vi.fn();
		const unregisterFirst = budget.registerSecondary({ onPause: first });
		budget.registerSecondary({ onPause: second }); // paused (index 1)
		unregisterFirst();
		expect(second).toHaveBeenLastCalledWith(false); // now index 0, admitted
	});
});
