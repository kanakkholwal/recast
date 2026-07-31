import { describe, expect, it } from "vitest";
import { safeNext } from "./redirect";

describe("safeNext", () => {
	it("keeps same-origin paths, query and hash included", () => {
		expect(safeNext("/dashboard/recasts?tab=all#top")).toBe("/dashboard/recasts?tab=all#top");
	});

	it("falls back when there is no next", () => {
		expect(safeNext(null)).toBe("/dashboard");
		expect(safeNext("")).toBe("/dashboard");
		expect(safeNext(undefined, "/onboarding/team")).toBe("/onboarding/team");
	});

	it("rejects off-site destinations", () => {
		expect(safeNext("https://evil.com")).toBe("/dashboard");
		expect(safeNext("//evil.com")).toBe("/dashboard");
		expect(safeNext("/\\evil.com")).toBe("/dashboard");
		expect(safeNext("javascript:alert(1)")).toBe("/dashboard");
	});

	it("rejects off-site destinations hidden behind characters the URL parser strips", () => {
		expect(safeNext("/\t/evil.com")).toBe("/dashboard");
		expect(safeNext(" //evil.com")).toBe("/dashboard");
		expect(safeNext("\n\rhttps://evil.com")).toBe("/dashboard");
	});
});
