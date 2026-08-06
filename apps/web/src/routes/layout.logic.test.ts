import { describe, expect, it } from "vitest";
import { isChromeless } from "./layout.logic";

describe("isChromeless", () => {
	// The editor is full-height and ships the EditorToolbar; the marketing navbar
	// stacked above it pushes the editor past the viewport. The drop surface is
	// the same page, so it goes chromeless too.
	it("drops the chrome across the whole video editor playground", () => {
		expect(isChromeless("/playground")).toBe(true);
		expect(isChromeless("/playground/edit")).toBe(true);
	});

	it("keeps the existing app-shell routes chromeless", () => {
		for (const p of [
			"/dashboard",
			"/dashboard/settings",
			"/admin/users",
			"/onboarding",
			"/share/abc",
			"/tools/screenshot-editor/edit",
			"/login",
			"/verify-email",
		]) {
			expect(isChromeless(p), p).toBe(true);
		}
	});

	it("keeps the chrome on marketing pages", () => {
		for (const p of ["/", "/features", "/pricing", "/tools", "/tools/mp4-to-gif", "/blog"]) {
			expect(isChromeless(p), p).toBe(false);
		}
	});
});
