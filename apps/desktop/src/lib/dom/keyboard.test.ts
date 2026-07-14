import { describe, expect, it } from "vitest";
import {
	OVERLAY_SELECTOR,
	isOverlayOpen,
	tagActivatesOnSpace,
} from "./keyboard";

// A ParentNode stub is enough: isOverlayOpen only ever calls querySelector.
function root(match: boolean): ParentNode {
	return { querySelector: () => (match ? ({} as Element) : null) } as unknown as ParentNode;
}

describe("tagActivatesOnSpace", () => {
	it("yields Space to natively activatable controls", () => {
		for (const tag of ["BUTTON", "INPUT", "SELECT", "TEXTAREA", "SUMMARY"]) {
			expect(tagActivatesOnSpace(tag)).toBe(true);
		}
	});

	it("is case insensitive", () => {
		expect(tagActivatesOnSpace("button")).toBe(true);
	});

	it("yields to an anchor only when it is a real link", () => {
		expect(tagActivatesOnSpace("A", true)).toBe(true);
		expect(tagActivatesOnSpace("A", false)).toBe(false);
	});

	// The regression this exists for: a div with role="button" has no Space
	// handler of its own, so standing down would swallow the key entirely.
	it("keeps Space for plain elements, including role=button divs", () => {
		expect(tagActivatesOnSpace("DIV")).toBe(false);
		expect(tagActivatesOnSpace("SPAN")).toBe(false);
	});
});

describe("isOverlayOpen", () => {
	it("reports an open layer", () => {
		expect(isOverlayOpen(root(true))).toBe(true);
	});

	it("reports a clear page", () => {
		expect(isOverlayOpen(root(false))).toBe(false);
	});

	it("covers both ui-package slots and hand-rolled modals", () => {
		expect(OVERLAY_SELECTOR).toContain('[data-slot="dialog-content"]');
		expect(OVERLAY_SELECTOR).toContain('[data-slot="select-content"]');
		// PresetPicker is hand-rolled and has no data-slot.
		expect(OVERLAY_SELECTOR).toContain('[role="dialog"]');
	});
});
