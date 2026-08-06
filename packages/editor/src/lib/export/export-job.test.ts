import { describe, expect, it } from "vitest";
import { collectTransferables, type ExportJob } from "./export-job";

// collectTransferables only reads bitmap fields, so the scene fields are stubbed.
const bmp = (): ImageBitmap => ({}) as ImageBitmap;

function job(over: Partial<ExportJob> = {}): ExportJob {
	return {
		base: {} as ExportJob["base"],
		timeMap: { spans: [], outputDuration: 0 },
		outputDurationSec: 0,
		fps: 30,
		quality: "high",
		videoUrl: "file:///v.mp4",
		backgroundImage: null,
		cursorSprites: null,
		camera: null,
		annotation: null,
		caption: null,
		...over,
	};
}

describe("collectTransferables", () => {
	it("returns nothing when the job carries no bitmaps", () => {
		expect(collectTransferables(job())).toEqual([]);
	});

	it("gathers background, cursor sprites, and annotation images", () => {
		const bg = bmp();
		const rest = bmp();
		const press = bmp();
		const img = bmp();
		const t = collectTransferables(
			job({
				backgroundImage: bg,
				cursorSprites: {
					rest,
					press,
					restHotspot: [0, 0],
					pressHotspot: [0, 0],
				},
				annotation: {
					annotations: [],
					meta: { width: 1920, height: 1080 },
					padding: 0,
					outputAspect: "source",
					zoomRegions: [],
					canvasPxW: 1920,
					canvasPxH: 1080,
					images: [["p.png", img]],
				},
			}),
		);
		expect(t).toHaveLength(4);
		expect(t).toEqual(expect.arrayContaining([bg, rest, press, img]));
	});

	it("dedupes a bitmap shared across cursor states (drag/rightPress → press → rest)", () => {
		const rest = bmp();
		const t = collectTransferables(
			job({
				cursorSprites: {
					rest,
					press: rest,
					rightPress: rest,
					drag: rest,
					restHotspot: [0, 0],
					pressHotspot: [0, 0],
				},
			}),
		);
		expect(t).toEqual([rest]);
	});
});
