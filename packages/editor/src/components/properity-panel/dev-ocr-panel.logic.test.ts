import { describe, expect, it } from "vitest";
import type { OcrProgress, OcrStats, ScreenStateSpan, VideoTextTimeline } from "$lib/ipc-types";
import {
	boxLabel,
	boxRect,
	etaLabel,
	etaSeconds,
	exportBodyFor,
	phaseDetail,
	phaseTitle,
	progressValue,
	regionLabel,
	spanAsText,
	spanGist,
	summaryRows,
	timelineToMarkdown,
} from "./dev-ocr-panel.logic";

function progress(p: Partial<OcrProgress>): OcrProgress {
	return { phase: "reading", done: 0, total: 0, found: 0, ...p };
}

describe("progressValue", () => {
	it("is a percentage of the phase's own units", () => {
		expect(progressValue(progress({ done: 6, total: 24 }))).toBe(25);
		expect(progressValue(progress({ done: 24, total: 24 }))).toBe(100);
	});

	it("is indeterminate when the phase has no countable total", () => {
		// The bar must not divide by zero and render NaN.
		expect(progressValue(progress({ done: 5, total: 0 }))).toBeNull();
		expect(progressValue(null)).toBeNull();
	});

	it("clamps an overshooting sampler past 100", () => {
		// The scan's total is estimated from the container duration, which can
		// undershoot the real frame count.
		expect(progressValue(progress({ phase: "sampling", done: 28, total: 27 }))).toBe(100);
	});
});

describe("phase copy", () => {
	it("names the phase and counts its own units", () => {
		expect(phaseTitle(progress({ phase: "sampling" }))).toBe("Scanning for frames that changed");
		expect(phaseDetail(progress({ phase: "sampling", done: 40, total: 90, found: 3 }))).toBe(
			"Frame 40 of 90 · 3 frames kept",
		);
		expect(phaseTitle(progress({ phase: "reading" }))).toBe("Reading text from frames");
		expect(phaseDetail(progress({ phase: "reading", done: 2, total: 24, found: 1 }))).toBe(
			"Frame 2 of 24 · 1 screen found",
		);
	});

	it("reports the download in megabytes, not frames", () => {
		expect(
			phaseDetail(progress({ phase: "downloading", done: 6_000_000, total: 12_000_000 })),
		).toBe("6.0 of 12.0 MB · first run only");
	});

	it("drops the count when the phase has no total yet", () => {
		expect(phaseDetail(progress({ phase: "sampling", done: 4, total: 0, found: 1 }))).toBe(
			"Frame 4 · 1 frame kept",
		);
	});
});

describe("etaSeconds", () => {
	it("extrapolates from the phase's own elapsed time", () => {
		// 4 frames in 2s is 500ms a frame, with 20 to go.
		expect(etaSeconds(2000, progress({ done: 4, total: 24 }))).toBe(10);
		expect(etaLabel(2000, progress({ done: 4, total: 24 }))).toBe("about 10s left");
	});

	it("says nothing until there is enough of the phase to extrapolate from", () => {
		// One frame in is noise, not an estimate, and a wild number reads as broken.
		expect(etaSeconds(500, progress({ done: 1, total: 24 }))).toBeNull();
		expect(etaLabel(500, progress({ done: 1, total: 24 }))).toBe("");
	});

	it("says nothing once the phase is done or uncountable", () => {
		expect(etaSeconds(5000, progress({ done: 24, total: 24 }))).toBeNull();
		expect(etaSeconds(5000, progress({ done: 5, total: 0 }))).toBeNull();
	});

	it("switches to minutes for a long read", () => {
		// 4 frames in 40s is 10s a frame, with 20 to go: 200s.
		expect(etaLabel(40_000, progress({ done: 4, total: 24 }))).toBe("about 4 min left");
	});
});

describe("summaryRows", () => {
	const stats: OcrStats = {
		durationSecs: 30,
		framesScanned: 90,
		framesRead: 12,
		elements: 47,
		sampleMs: 800,
		modelLoadMs: 210,
		ocrMs: 4680,
	};

	it("shows what was read out of what was scanned, and the per-frame cost", () => {
		const rows = summaryRows(stats, 5);
		const byLabel = Object.fromEntries(rows.map((r) => [r.label, r.value]));
		expect(byLabel["Frames read"]).toBe("12 of 90");
		expect(byLabel["Screen states"]).toBe("5");
		expect(byLabel["Text elements"]).toBe("47");
		expect(byLabel["Scan"]).toBe("800ms");
		expect(byLabel["Read"]).toBe("4.7s · 390ms/frame");
	});

	it("does not divide by zero when nothing was read", () => {
		const empty = { ...stats, framesRead: 0, ocrMs: 0 };
		const read = summaryRows(empty, 0).find((r) => r.label === "Read");
		expect(read?.value).toBe("0ms · 0ms/frame");
	});
});

describe("element geometry", () => {
	it("turns a normalized box into a percentage rect", () => {
		expect(boxRect([0.1, 0.2, 0.5, 0.4])).toEqual({ left: 10, top: 20, width: 40, height: 20 });
	});

	it("names the region a reader can picture", () => {
		expect(regionLabel([0.0, 0.0, 0.2, 0.1])).toBe("top left");
		expect(regionLabel([0.8, 0.85, 1.0, 0.95])).toBe("bottom right");
		expect(regionLabel([0.4, 0.4, 0.6, 0.6])).toBe("center");
		// A band that spans the middle row reads by its column alone.
		expect(regionLabel([0.0, 0.45, 0.2, 0.55])).toBe("left");
		// ...and one centred horizontally reads by its row alone.
		expect(regionLabel([0.4, 0.0, 0.6, 0.1])).toBe("top");
	});

	it("writes the box as readable spans", () => {
		expect(boxLabel([0.12, 0.08, 0.44, 0.12])).toBe("x 12–44% · y 8–12%");
	});
});

describe("span text", () => {
	const span: ScreenStateSpan = {
		start: 1,
		end: 4,
		preview: null,
		elements: [
			{ id: 0, kind: "text", bbox: [0.1, 0.05, 0.4, 0.1], content: "Export Settings", source: "ocrs" },
			{ id: 1, kind: "text", bbox: [0.1, 0.2, 0.3, 0.25], content: "Frame rate 60fps", source: "ocrs" },
		],
	};
	const tc = (t: number) => `0:0${t}`;

	it("gists the span for the list row", () => {
		expect(spanGist(span)).toBe("Export Settings · Frame rate 60fps");
		expect(spanGist({ ...span, elements: [] })).toBe("No text read");
	});

	it("copies as readable lines, not JSON", () => {
		const text = spanAsText(span, tc);
		expect(text).toContain("Screen at 0:01 to 0:04 (2 elements)");
		expect(text).toContain('0. "Export Settings"  [top left · x 10–40% · y 5–10%]');
		expect(text).not.toContain("{");
	});
});

describe("export", () => {
	const timeline: VideoTextTimeline = {
		engine: "ocrs",
		stats: {
			durationSecs: 4,
			framesScanned: 12,
			framesRead: 2,
			elements: 2,
			sampleMs: 100,
			modelLoadMs: 30,
			ocrMs: 700,
		},
		spans: [
			{
				start: 1,
				end: 4,
				preview: "data:image/jpeg;base64,AAAA",
				elements: [
					{ id: 0, kind: "text", bbox: [0.1, 0.05, 0.4, 0.1], content: "Export Settings", source: "ocrs" },
				],
			},
			{ start: 4, end: 4, preview: null, elements: [] },
		],
	};
	const tc = (t: number) => `0:0${Math.round(t)}`;

	it("writes Markdown a person can read, with the frame embedded", () => {
		const md = timelineToMarkdown(timeline, tc);
		expect(md).toContain("# Screen text");
		expect(md).toContain("ocrs · 2 of 12 frames read · 2 screen states · 2 elements");
		expect(md).toContain("## 0:01 – 0:04");
		// The preview data URI rides inside the file, so the image travels with it.
		expect(md).toContain("![Frame at 0:01](data:image/jpeg;base64,AAAA)");
		expect(md).toContain('- **0** "Export Settings" — top left (x 10–40% · y 5–10%)');
		// A frame with no text still gets a section, explaining why it was kept.
		expect(md).toContain("_No text read; kept because the picture changed._");
	});

	it("picks format by the chosen file extension, JSON as the lossless default", () => {
		expect(exportBodyFor("C:/tmp/read.md", timeline, tc)).toContain("# Screen text");
		// JSON is the exact timeline, previews included, so nothing is lost.
		const json = exportBodyFor("C:/tmp/read.json", timeline, tc);
		expect(JSON.parse(json)).toEqual(timeline);
		// Anything unrecognized falls back to JSON rather than guessing.
		expect(() => JSON.parse(exportBodyFor("C:/tmp/read.txt", timeline, tc))).not.toThrow();
	});
});
