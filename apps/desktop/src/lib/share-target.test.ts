import { describe, expect, it, vi } from "vitest";

// @recast/icons re-exports a barrel that can't resolve under node, and only identity matters here.
vi.mock("@recast/icons", () => ({
	__esModule: true,
	default: {},
	...Object.fromEntries(
		["BrandApple", "BrandLinux", "BrandWindows", "Share2"].map((name) => [name, () => name]),
	),
}));

const { shareTargetFor } = await import("./share-target");
const icons = await import("@recast/icons");

describe("shareTargetFor", () => {
	it("marks each desktop platform with its own logo", () => {
		expect(shareTargetFor("windows").icon).toBe(icons.BrandWindows);
		expect(shareTargetFor("macos").icon).toBe(icons.BrandApple);
		expect(shareTargetFor("linux").icon).toBe(icons.BrandLinux);
	});

	// Tauri's Platform union says 'macos', never 'darwin', so a darwin-keyed mapping falls through on every Mac.
	it("does not key macOS off 'darwin'", () => {
		expect(shareTargetFor("darwin" as never).icon).toBe(icons.Share2);
		expect(shareTargetFor("macos").icon).not.toBe(icons.Share2);
	});

	it("groups iOS with macOS", () => {
		expect(shareTargetFor("ios").icon).toBe(icons.BrandApple);
	});

	// A platform we have no vouched-for mark for must not borrow another's.
	it("falls back to a neutral mark for anything else", () => {
		for (const p of ["freebsd", "android", "solaris"] as const) {
			expect(shareTargetFor(p).icon).toBe(icons.Share2);
		}
		expect(shareTargetFor(null).icon).toBe(icons.Share2);
		expect(shareTargetFor(undefined).icon).toBe(icons.Share2);
	});

	it("labels every target", () => {
		for (const p of ["windows", "macos", "linux", "android"] as const) {
			expect(shareTargetFor(p).label.trim()).not.toBe("");
		}
	});
});
