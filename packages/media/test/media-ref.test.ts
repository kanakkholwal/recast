import { describe, expect, it, vi } from "vitest";
import { mediaRefExtension, mediaRefKey, toMediaRef } from "../src/media-ref";

describe("toMediaRef", () => {
	it("wraps a bare URL", () => {
		expect(toMediaRef("asset://localhost/clip.mp4")).toEqual({
			kind: "url",
			url: "asset://localhost/clip.mp4",
		});
	});

	it("wraps a Blob", () => {
		const blob = new Blob(["x"], { type: "video/mp4" });
		expect(toMediaRef(blob)).toEqual({ kind: "blob", blob });
	});

	it("passes an existing ref through", () => {
		const ref = { kind: "blob", blob: new Blob(["x"]) } as const;
		expect(toMediaRef(ref)).toBe(ref);
	});
});

describe("mediaRefExtension", () => {
	it("reads the extension past a query and hash", () => {
		expect(mediaRefExtension(toMediaRef("https://cdn/a/clip.webm?v=2#t=1"))).toBe("webm");
	});

	it("is empty for an object URL, so the guard cannot false-positive", () => {
		expect(mediaRefExtension(toMediaRef("blob:http://localhost:4420/9f2c-4a"))).toBe("");
	});

	it("is empty when the last segment has no extension", () => {
		expect(mediaRefExtension(toMediaRef("https://cdn/stream/master"))).toBe("");
	});

	it("reads a File's own name", () => {
		const file = new File(["x"], "My Recording.MP4", { type: "video/mp4" });
		expect(mediaRefExtension(toMediaRef(file))).toBe("mp4");
	});

	it("falls back to the MIME subtype for an unnamed Blob", () => {
		expect(mediaRefExtension(toMediaRef(new Blob(["x"], { type: "video/webm" })))).toBe("webm");
	});

	// A dotted name with no real extension must not be mistaken for one, or an unrelated suffix matches the unsupported list.
	it("ignores a suffix that is not extension-shaped", () => {
		const file = new File(["x"], "clip.finalversion", { type: "video/mp4" });
		expect(mediaRefExtension(toMediaRef(file))).toBe("mp4");
	});
});

describe("mediaRefKey", () => {
	it("scopes a blob by identity, not by a per-open object URL", () => {
		const of = () =>
			toMediaRef(new File(["abc"], "clip.mp4", { type: "video/mp4", lastModified: 1700 }));
		expect(mediaRefKey(of())).toBe(mediaRefKey(of()));
	});

	it("separates two files that differ only in size", () => {
		const a = new File(["abc"], "clip.mp4", { lastModified: 1700 });
		const b = new File(["abcd"], "clip.mp4", { lastModified: 1700 });
		expect(mediaRefKey(toMediaRef(a))).not.toBe(mediaRefKey(toMediaRef(b)));
	});
});

describe("mediaRefSource", () => {
	it("slices a Blob and range-requests a URL", async () => {
		vi.doMock("mediabunny", () => ({
			BlobSource: class {
				readonly tag = "blob";
				constructor(readonly blob: Blob) {}
			},
			UrlSource: class {
				readonly tag = "url";
				constructor(readonly url: string) {}
			},
		}));
		const { mediaRefSource } = await import("../src/mediabunny");

		const payload = new Blob(["x"]);
		expect(mediaRefSource({ kind: "blob", blob: payload })).toMatchObject({
			tag: "blob",
			blob: payload,
		});
		expect(mediaRefSource({ kind: "url", url: "asset://x.mp4" })).toMatchObject({
			tag: "url",
			url: "asset://x.mp4",
		});
	});
});
