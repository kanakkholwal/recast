import { describe, expect, it } from "vitest";
import { repoPath, sourceUrl } from "./source-links";

describe("repoPath", () => {
	it("passes a repo-rooted path through", () => {
		expect(repoPath("packages/media/src/playback/worker.ts")).toBe(
			"packages/media/src/playback/worker.ts",
		);
	});

	it("resolves a Rust module path against the crate root", () => {
		expect(repoPath("recording/mod.rs")).toBe("apps/desktop/src-tauri/src/recording/mod.rs");
	});

	it("resolves a file that sits in the crate root", () => {
		expect(repoPath("ffmpeg.rs")).toBe("apps/desktop/src-tauri/src/ffmpeg.rs");
	});

	// `mod.rs` names a dozen real files, so linking it would send readers to the wrong one; it stays plain code.
	it("refuses a bare filename that could be any of several files", () => {
		expect(repoPath("mod.rs")).toBeNull();
	});

	it("refuses a bare TypeScript filename", () => {
		expect(repoPath("time-map.ts")).toBeNull();
	});

	it("refuses a directory that is not a known Rust module", () => {
		expect(repoPath("vendor/thing.rs")).toBeNull();
	});

	it("refuses something that is not a source file at all", () => {
		expect(repoPath("RenderCore")).toBeNull();
	});

	it("refuses a path that escapes the repo", () => {
		expect(repoPath("../../etc/passwd.ts")).toBeNull();
	});

	it("refuses an absolute path", () => {
		expect(repoPath("/etc/passwd.ts")).toBeNull();
	});

	it("ignores surrounding whitespace", () => {
		expect(repoPath("  commands/cloud.rs  ")).toBe("apps/desktop/src-tauri/src/commands/cloud.rs");
	});
});

describe("sourceUrl", () => {
	it("points at the file on the default branch", () => {
		expect(sourceUrl("render/ops.rs")).toBe(
			"https://github.com/kanakkholwal/recast/blob/main/apps/desktop/src-tauri/src/render/ops.rs",
		);
	});

	it("gives nothing back for an unlinkable reference", () => {
		expect(sourceUrl("mod.rs")).toBeNull();
	});

	// Line numbers were stripped from the docs, and a stray one must not link to a file that doesn't exist.
	it("gives nothing back for a path that still carries a line number", () => {
		expect(sourceUrl("recording/mod.rs:541")).toBeNull();
	});
});
