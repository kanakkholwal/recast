import { describe, expect, it } from "vitest";
import { destinationTile, type UploadPhase, uploadForPath } from "./destination-tile";

const LABELS = { idle: "Recast Cloud", done: "Copy link" };

function tile(input: { checking?: boolean; phase?: UploadPhase; hasRecord?: boolean }) {
	return destinationTile(LABELS, {
		checking: input.checking ?? false,
		phase: input.phase,
		hasRecord: input.hasRecord ?? false,
	});
}

describe("destinationTile", () => {
	it("starts idle and clickable", () => {
		expect(tile({})).toEqual({ status: "idle", label: "Recast Cloud", disabled: false });
	});

	it("locks out repeat clicks during the pre-flight check and the upload", () => {
		expect(tile({ checking: true }).disabled).toBe(true);
		expect(tile({ phase: "uploading" }).disabled).toBe(true);
	});

	it("reports the pre-flight check before any upload exists", () => {
		expect(tile({ checking: true })).toEqual({
			status: "busy",
			label: "Checking…",
			disabled: true,
		});
	});

	it("offers a retry after a failure", () => {
		expect(tile({ phase: "error" })).toEqual({ status: "error", label: "Retry", disabled: false });
	});

	it("returns to idle after a cancel so the upload can be restarted", () => {
		expect(tile({ phase: "cancelled" })).toEqual({
			status: "idle",
			label: "Recast Cloud",
			disabled: false,
		});
	});

	it("stays actionable once done, so the link can be copied", () => {
		expect(tile({ phase: "complete" })).toEqual({
			status: "done",
			label: "Copy link",
			disabled: false,
		});
		expect(tile({ hasRecord: true }).status).toBe("done");
	});

	it("lets a live upload outrank a record from a previous run", () => {
		expect(tile({ phase: "uploading", hasRecord: true }).status).toBe("busy");
		expect(tile({ phase: "error", hasRecord: true }).status).toBe("error");
	});
});

describe("uploadForPath", () => {
	const make = (id: string, sourcePath: string, status: UploadPhase) => ({
		id,
		sourcePath,
		status,
	});

	it("ignores uploads of other files", () => {
		const uploads = { a: make("a", "/other.mp4", "uploading") };
		expect(uploadForPath(uploads, "/clip.mp4")).toBeUndefined();
	});

	it("prefers the in-flight upload over an older completed one", () => {
		const uploads = {
			a: make("a", "/clip.mp4", "complete"),
			b: make("b", "/clip.mp4", "uploading"),
		};
		expect(uploadForPath(uploads, "/clip.mp4")?.id).toBe("b");
	});

	it("falls back to the most recent terminal upload", () => {
		const uploads = {
			a: make("a", "/clip.mp4", "complete"),
			b: make("b", "/clip.mp4", "error"),
		};
		expect(uploadForPath(uploads, "/clip.mp4")?.id).toBe("b");
	});
});
