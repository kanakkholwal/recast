import { describe, expect, it } from "vitest";
import { planReconcile } from "./reconcile";

describe("planReconcile", () => {
	it("skips when the echo is our own write", () => {
		const state = { trimStart: 0, trimEnd: 10 };
		expect(planReconcile({ current: state, incoming: { ...state }, dirty: false })).toEqual({
			action: "skip",
			reason: "identical",
		});
	});

	it("skips an identical echo even while dirty, so a save can't self-trigger", () => {
		const state = { trimEnd: 10 };
		expect(planReconcile({ current: state, incoming: { ...state }, dirty: true }).action).toBe(
			"skip",
		);
	});

	it("skips when the only difference is null-vs-omitted", () => {
		expect(
			planReconcile({
				current: { trimEnd: 10, lastAppliedPresetId: null },
				incoming: { trimEnd: 10 },
				dirty: false,
			}).action,
		).toBe("skip");
	});

	it("adopts a clean divergence", () => {
		expect(
			planReconcile({ current: { trimEnd: 10 }, incoming: { trimEnd: 12 }, dirty: false }),
		).toEqual({ action: "apply" });
	});

	it("refuses to clobber unsaved edits", () => {
		expect(
			planReconcile({ current: { trimEnd: 10 }, incoming: { trimEnd: 12 }, dirty: true }),
		).toEqual({ action: "conflict" });
	});
});
