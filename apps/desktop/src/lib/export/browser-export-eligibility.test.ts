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

	it("blocks GIF (2-pass palette stays on Rust)", () => {
		expect(browserExportBlockedReason(store({ exportFormat: "gif" }))).toBe("gif");
	});

	it("blocks burned captions but not sidecar-only", () => {
		expect(
			browserExportBlockedReason(
				store({ transcript, captionExport: { burnIn: true, sidecar: "none" } }),
			),
		).toBe("burned captions");
		// Sidecar without burn-in doesn't block (only pixels burned into the video do).
		expect(
			browserExportBlockedReason(
				store({ transcript, captionExport: { burnIn: false, sidecar: "none" } }),
			),
		).toBeNull();
	});

	it("does not block burn-in when there's no transcript", () => {
		expect(
			browserExportBlockedReason(store({ captionExport: { burnIn: true, sidecar: "none" } })),
		).toBeNull();
	});

	it("blocks a project with a visible annotation", () => {
		expect(browserExportBlockedReason(store({ annotations: [{ hidden: false }] }))).toBe(
			"annotations",
		);
	});

	it("ignores hidden / globally-hidden annotations", () => {
		expect(browserExportBlockedReason(store({ annotations: [{ hidden: true }] }))).toBeNull();
		expect(
			browserExportBlockedReason(
				store({ annotationsGloballyHidden: true, annotations: [{ hidden: false }] }),
			),
		).toBeNull();
	});
});
