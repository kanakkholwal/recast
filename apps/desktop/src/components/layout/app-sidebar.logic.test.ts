import { describe, expect, it } from "vitest";
import { isActive } from "./app-sidebar.logic";

describe("isActive", () => {
	it("matches a nav item on its own route", () => {
		expect(isActive("/recasts", "/recasts")).toBe(true);
		expect(isActive("/exports", "/exports")).toBe(true);
	});

	it("stays active on a child route", () => {
		expect(isActive("/settings", "/settings/cloud")).toBe(true);
	});

	// A bare startsWith would light up "Exports" for /exports-archive, so two nav
	// items could read as current at once.
	it("only matches at a segment boundary", () => {
		expect(isActive("/exports", "/exports-archive")).toBe(false);
		expect(isActive("/recasts", "/recastsfoo")).toBe(false);
	});

	// Home is a prefix of every route, so it can never prefix-match.
	it("matches home only exactly", () => {
		expect(isActive("/", "/")).toBe(true);
		expect(isActive("/", "/recasts")).toBe(false);
		expect(isActive("/", "/settings/cloud")).toBe(false);
	});

	it("does not match an unrelated route", () => {
		expect(isActive("/recasts", "/exports")).toBe(false);
	});
});
