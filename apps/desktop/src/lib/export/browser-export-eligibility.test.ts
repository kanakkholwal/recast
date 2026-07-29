import { describe, expect, it } from "vitest";
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { browserExportBlockedReason } from "./browser-export-eligibility";

// Minimal store shape for browserExportBlockedReason + its buildCaptionExport call.
function store(overrides: Record<string, unknown> = {}): EditorStore {
	return {
		exportFormat: "mp4",
		transcript: null,
		captionExport: { burnIn: false, sidecar: "none" },
		annotationsGloballyHidden: false,
		annotations: [],
		...overrides,
	} as unknown as EditorStore;
}

const transcript = { segments: [{ id: "a", start: 0, end: 1, text: "hi", words: [] }] };

describe("browserExportBlockedReason", () => {
	it("is eligible (null) for a plain screen recording", () => {
		expect(browserExportBlockedReason(store())).toBeNull();
	});

	it("allows GIF (browser composites; Rust runs only the palette)", () => {
		expect(browserExportBlockedReason(store({ exportFormat: "gif" }))).toBeNull();
	});

	it("allows burned captions (the browser burns them now)", () => {
		expect(
			browserExportBlockedReason(
				store({ transcript, captionExport: { burnIn: true, sidecar: "none" } }),
			),
		).toBeNull();
	});

	it("allows every annotation kind — the browser draws them all now", () => {
		for (const kind of ["rect", "ellipse", "arrow", "image", "text", "blur"]) {
			expect(
				browserExportBlockedReason(store({ annotations: [{ hidden: false, kind: { kind } }] })),
			).toBeNull();
		}
	});
});
