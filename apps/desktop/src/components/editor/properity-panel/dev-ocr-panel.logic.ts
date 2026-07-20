/**
 * Pure presentation logic for the screen-text (OCR) panel.
 *
 * Kept out of the component so the read's progress arithmetic and the geometry that
 * places boxes over a preview frame are unit-testable without mounting anything.
 */

import type {
	OcrProgress,
	OcrStats,
	ScreenElement,
	ScreenStateSpan,
	VideoTextTimeline,
} from "$lib/ipc-types";

/** What the panel is doing. `idle` and `error` are panel states; the rest mirror the backend's phases. */
export type RunStatus = "idle" | "running" | "ready" | "error";

/** A progress bar's value, or null when the phase has nothing countable to show yet. */
export function progressValue(p: OcrProgress | null): number | null {
	if (!p || p.total <= 0) return null;
	const pct = (p.done / p.total) * 100;
	// The sampler's total is estimated from the container duration, which can
	// undershoot the real frame count. Clamp rather than render a 104% bar.
	return Math.min(100, Math.max(0, pct));
}

/** Headline for the current phase. */
export function phaseTitle(p: OcrProgress | null): string {
	switch (p?.phase) {
		case "downloading":
			return "Fetching OCR models";
		case "sampling":
			return "Scanning for frames that changed";
		case "reading":
			return "Reading text from frames";
		case "done":
			return "Finishing up";
		default:
			return "Starting";
	}
}

/**
 * The counted detail under the headline: what unit this phase is counting, how far
 * it has got, and what it has produced so far. This is the line that turns a
 * spinner into visible work.
 */
export function phaseDetail(p: OcrProgress | null): string {
	if (!p) return "";
	switch (p.phase) {
		case "downloading":
			return p.total > 0 ? `${mb(p.done)} of ${mb(p.total)} MB · first run only` : "First run only";
		case "sampling": {
			const of = p.total > 0 ? `Frame ${p.done} of ${p.total}` : `Frame ${p.done}`;
			return `${of} · ${plural(p.found, "frame")} kept`;
		}
		case "reading": {
			const of = p.total > 0 ? `Frame ${p.done} of ${p.total}` : `Frame ${p.done}`;
			return `${of} · ${plural(p.found, "screen")} found`;
		}
		default:
			return "";
	}
}

/**
 * Seconds left in this phase, from its own elapsed time and its own units. Null
 * until there is enough of the phase behind us to extrapolate from: an estimate off
 * the first frame or two swings wildly and reads as broken.
 */
export function etaSeconds(phaseElapsedMs: number, p: OcrProgress | null): number | null {
	if (!p || p.total <= 0 || p.done < MIN_UNITS_FOR_ETA) return null;
	if (p.done >= p.total || phaseElapsedMs <= 0) return null;
	const perUnit = phaseElapsedMs / p.done;
	return Math.ceil(((p.total - p.done) * perUnit) / 1000);
}

/** Below this many completed units, an extrapolation is noise, not an estimate. */
const MIN_UNITS_FOR_ETA = 3;

/** "about 12s left", or "" when there is nothing trustworthy to say. */
export function etaLabel(phaseElapsedMs: number, p: OcrProgress | null): string {
	const secs = etaSeconds(phaseElapsedMs, p);
	if (secs === null) return "";
	if (secs < 60) return `about ${secs}s left`;
	return `about ${Math.ceil(secs / 60)} min left`;
}

/**
 * The run summary, as label/value rows. Explains where the time went and how much
 * of the video was actually looked at, so a slow or thin read can be attributed
 * instead of guessed at.
 */
export function summaryRows(
	stats: OcrStats,
	spans: number,
): Array<{ label: string; value: string; hint: string }> {
	const perFrame = stats.framesRead > 0 ? Math.round(stats.ocrMs / stats.framesRead) : 0;
	return [
		{
			label: "Frames read",
			value: `${stats.framesRead} of ${stats.framesScanned}`,
			hint: "Frames that changed enough to be worth reading, out of every frame the scan walked.",
		},
		{
			label: "Screen states",
			value: `${spans}`,
			hint: "Runs of time where the screen text stayed the same. Neighbouring frames that read the same collapse into one.",
		},
		{
			label: "Text elements",
			value: `${stats.elements}`,
			hint: "Recognized lines of text across every screen state.",
		},
		{
			label: "Scan",
			value: secs(stats.sampleMs),
			hint: "Decoding the video and deciding which frames changed.",
		},
		{
			label: "Model load",
			value: secs(stats.modelLoadMs),
			hint: "Loading the detection and recognition models. Paid once per run.",
		},
		{
			label: "Read",
			value: `${secs(stats.ocrMs)} · ${perFrame}ms/frame`,
			hint: "The OCR itself. It dominates the run, and is far slower in a debug build than in release.",
		},
	];
}

/** Percentage rect for drawing an element's box over a preview image. */
export function boxRect(bbox: ScreenElement["bbox"]): {
	left: number;
	top: number;
	width: number;
	height: number;
} {
	const [x0, y0, x1, y1] = bbox;
	return {
		left: pct(x0),
		top: pct(y0),
		width: pct(Math.max(0, x1 - x0)),
		height: pct(Math.max(0, y1 - y0)),
	};
}

/** Inline style for an element's box overlay, in percentages of the frame. */
export function boxStyle(bbox: ScreenElement["bbox"]): string {
	const r = boxRect(bbox);
	return `left:${r.left}%;top:${r.top}%;width:${r.width}%;height:${r.height}%`;
}

/**
 * Where on screen an element sits, in words. The raw box is four normalized floats,
 * which nobody can picture; "top left" is the thing a reader actually wants from it.
 */
export function regionLabel(bbox: ScreenElement["bbox"]): string {
	const [x0, y0, x1, y1] = bbox;
	const cx = (x0 + x1) / 2;
	const cy = (y0 + y1) / 2;
	const row = cy < 1 / 3 ? "top" : cy < 2 / 3 ? "middle" : "bottom";
	const col = cx < 1 / 3 ? "left" : cx < 2 / 3 ? "center" : "right";
	if (row === "middle" && col === "center") return "center";
	if (row === "middle") return col;
	if (col === "center") return row;
	return `${row} ${col}`;
}

/** An element's box as readable spans, e.g. "x 12–44% · y 8–12%". */
export function boxLabel(bbox: ScreenElement["bbox"]): string {
	const [x0, y0, x1, y1] = bbox;
	return `x ${pct(x0)}–${pct(x1)}% · y ${pct(y0)}–${pct(y1)}%`;
}

/** One-line gist of a span for the list row. */
export function spanGist(span: ScreenStateSpan): string {
	const text = span.elements
		.map((e) => e.content.trim())
		.filter(Boolean)
		.join(" · ");
	return text || "No text read";
}

/**
 * The span as a plain, copyable block: what an agent would be handed, written the
 * way a person reads it. Deliberately not JSON, which is the shape the panel exists
 * to make legible in the first place.
 */
export function spanAsText(span: ScreenStateSpan, timecode: (t: number) => string): string {
	const head = `Screen at ${timecode(span.start)} to ${timecode(span.end)} (${plural(span.elements.length, "element")})`;
	const lines = span.elements.map(
		(e) => `  ${e.id}. "${e.content}"  [${regionLabel(e.bbox)} · ${boxLabel(e.bbox)}]`,
	);
	return [head, ...lines].join("\n");
}

/**
 * The whole read as Markdown: a summary line, then one section per screen state
 * with its frame embedded (previews are data URIs, so the image travels inside the
 * file) and its elements as a list. This is the human-and-portable export; the JSON
 * export is the lossless machine one. Both are offered so a downstream consumer can
 * take whichever it wants.
 */
export function timelineToMarkdown(
	timeline: VideoTextTimeline,
	timecode: (t: number) => string,
): string {
	const { stats, spans, engine } = timeline;
	const head = [
		"# Screen text",
		"",
		`${engine} · ${stats.framesRead} of ${stats.framesScanned} frames read · ${plural(spans.length, "screen state")} · ${plural(stats.elements, "element")}`,
	];
	const body = spans.map((span) => {
		const lines = [
			`## ${timecode(span.start)} – ${timecode(span.end)}`,
			"",
			...(span.preview ? [`![Frame at ${timecode(span.start)}](${span.preview})`, ""] : []),
		];
		if (span.elements.length === 0) {
			lines.push("_No text read; kept because the picture changed._");
		} else {
			for (const el of span.elements) {
				lines.push(`- **${el.id}** "${el.content}" — ${regionLabel(el.bbox)} (${boxLabel(el.bbox)})`);
			}
		}
		return lines.join("\n");
	});
	return [...head, "", ...body].join("\n").concat("\n");
}

/** Default filename for an export, by format. */
export function exportFilename(format: "json" | "md"): string {
	return `screen-text.${format}`;
}

/** Serialize the read for `dest`, picking format by its extension. Falls back to
 *  JSON, the lossless shape, for any unrecognized extension. */
export function exportBodyFor(
	dest: string,
	timeline: VideoTextTimeline,
	timecode: (t: number) => string,
): string {
	return dest.toLowerCase().endsWith(".md")
		? timelineToMarkdown(timeline, timecode)
		: JSON.stringify(timeline, null, 2);
}

function pct(v: number): number {
	return Math.round(Math.min(1, Math.max(0, v)) * 100);
}

function mb(bytes: number): string {
	return (bytes / 1_000_000).toFixed(1);
}

function secs(ms: number): string {
	return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}

function plural(n: number, noun: string): string {
	return `${n} ${noun}${n === 1 ? "" : "s"}`;
}
