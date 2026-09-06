import { describe, expect, it } from "vitest";
import type { EditorRenderState } from "../../stores/editor-store.svelte";
import { exportPayload, exportTimeMap, type RunExportOptions } from "./export-payload";

function options(overrides: Partial<RunExportOptions> = {}): RunExportOptions {
	return {
		inputPath: "in.mp4",
		format: "mp4",
		quality: "source",
		renderState: {} as EditorRenderState,
		exportId: "e1",
		...overrides,
	};
}

describe("exportPayload", () => {
	// Rust reads `engineExport`: renaming it silently leaves every export on the FFmpeg compositor, failing at neither end.
	it("carries the engine-export flag", () => {
		expect(exportPayload(options({ engineExport: true })).engineExport).toBe(true);
	});

	it("defaults the engine-export flag off when the caller omits it", () => {
		expect(exportPayload(options()).engineExport).toBe(false);
	});

	// An omitted optional must arrive as an explicit null, or serde's default decides instead of the caller.
	it("sends the optional fields explicitly rather than dropping them", () => {
		const payload = exportPayload(options());
		expect(payload).toHaveProperty("browserVideoPath", null);
		expect(payload).toHaveProperty("timeMap", null);
		expect(payload).toHaveProperty("captionSidecar", null);
		expect(payload.burnCaptions).toBe(false);
	});

	it("flattens the caption payload into the two fields the backend reads", () => {
		const payload = exportPayload(options({ captions: { burnCaptions: true, sidecar: null } }));
		expect(payload.burnCaptions).toBe(true);
		expect(payload).not.toHaveProperty("captions");
	});
});

describe("exportTimeMap", () => {
	// The backend replays these spans rather than re-deriving them, so a lost field is a differently-timed export, not a crash.
	it("keeps each span's bounds and speed", () => {
		const spans = exportTimeMap({
			spans: [{ origStart: 1, origEnd: 2, speed: 1.5 }],
		});
		expect(spans).toEqual([{ origStart: 1, origEnd: 2, speed: 1.5 }]);
	});
});
