/**
 * Hybrid-raster export path for text annotations.
 *
 * Rust has no font rasterizer, so each text annotation is rendered to a
 * transparent PNG by the WebView and sent across IPC as a data URL on a
 * synthetic image-kind annotation. Invoked from handleExport before the
 * renderState reaches `invoke("enqueue_export", ...)`; non-text passes through.
 */
import type { Annotation } from "../../stores/editor-store.svelte";

/**
 * Replace every text annotation with an image annotation whose `path` is a
 * `data:image/png;base64,…` URL of the pre-rendered transparent PNG.
 *
 * @param canvasWidth  Pixel width of the export canvas (source.width + 2*padding).
 */
export async function expandTextAnnotations<T extends Pick<Annotation, "kind">>(
	annotations: T[],
	canvasWidth: number,
	canvasHeight: number,
): Promise<T[]> {
	if (canvasWidth <= 0 || canvasHeight <= 0) return annotations;
	// Wait for webfonts, so a text annotation doesn't bake in a fallback font it was never previewed with.
	if (typeof document !== "undefined" && document.fonts?.ready) {
		try {
			await document.fonts.ready;
		} catch {
			// Font readiness is best-effort; render what we have.
		}
	}
	const out: T[] = [];
	for (const a of annotations) {
		if (a.kind.kind !== "text") {
			out.push(a);
			continue;
		}
		const k = a.kind;
		const rendered = renderTextToDataUrl(k, canvasWidth, canvasHeight);
		if (!rendered) {
			// Drop the annotation rather than fail the whole export.
			console.warn("rasterize-text: failed to render text annotation, skipping", k.content);
			continue;
		}
		out.push({
			...a,
			kind: {
				kind: "image",
				x: Math.min(k.x, k.x + k.w),
				y: Math.min(k.y, k.y + k.h),
				w: Math.abs(k.w),
				// Grow to the rendered content height so overflow isn't clipped, matching the preview's min-height box.
				h: rendered.heightPx / canvasHeight,
				path: rendered.url,
				opacity: 1,
				radius: 0,
			},
		} as T);
	}
	return out;
}

function renderTextToDataUrl(
	k: Extract<Annotation["kind"], { kind: "text" }>,
	canvasWidth: number,
	canvasHeight: number,
): { url: string; heightPx: number } | null {
	// UV box → export-canvas pixels.
	const boxW = Math.max(1, Math.round(Math.abs(k.w) * canvasWidth));
	const boxH = Math.max(1, Math.round(Math.abs(k.h) * canvasHeight));
	const fontPx = Math.max(1, Math.round(k.fontSize * canvasHeight));
	const font = `${k.fontWeight} ${fontPx}px ${k.fontFamily}`;

	// Wrap on a scratch context first so the canvas can be sized to the content, as the preview's min-height does.
	const scratch = document.createElement("canvas").getContext("2d");
	if (!scratch) return null;
	scratch.font = font;
	const lines = wrapText(scratch, k.content, boxW);
	const lineHeightPx = fontPx * Math.max(1, k.lineHeight);
	const outH = Math.max(boxH, Math.ceil(lines.length * lineHeightPx));

	const canvas = document.createElement("canvas");
	canvas.width = boxW;
	canvas.height = outH;
	const ctx = canvas.getContext("2d");
	if (!ctx) return null;

	ctx.clearRect(0, 0, boxW, outH);
	ctx.font = font;
	ctx.fillStyle = k.color;
	// Centre each line in its line box (half-leading), matching the preview's CSS line-height.
	ctx.textBaseline = "middle";
	ctx.textAlign = k.align === "center" ? "center" : k.align === "right" ? "right" : "left";
	const xAnchor = k.align === "center" ? boxW / 2 : k.align === "right" ? boxW - 1 : 0;

	for (let i = 0; i < lines.length; i++) {
		ctx.fillText(lines[i], xAnchor, (i + 0.5) * lineHeightPx);
	}

	return { url: canvas.toDataURL("image/png"), heightPx: outH };
}

/**
 * Greedy word-wrap that respects explicit "\n" line breaks. A single word wider
 * than `maxWidth` is broken character by character (matching the preview's CSS
 * `wrap-break-word`), rather than overflowing on its own line.
 */
function wrapText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string[] {
	const lines: string[] = [];
	for (const paragraph of text.split(/\r?\n/)) {
		if (paragraph.length === 0) {
			lines.push("");
			continue;
		}
		const words = paragraph.split(/\s+/);
		let current = "";
		for (const word of words) {
			if (ctx.measureText(word).width <= maxWidth) {
				const candidate = current ? `${current} ${word}` : word;
				if (ctx.measureText(candidate).width <= maxWidth || current === "") {
					current = candidate;
				} else {
					lines.push(current);
					current = word;
				}
			} else {
				// Over-long word: flush the line, then pack its characters into lines.
				if (current) {
					lines.push(current);
					current = "";
				}
				for (const ch of word) {
					if (current && ctx.measureText(current + ch).width > maxWidth) {
						lines.push(current);
						current = ch;
					} else {
						current += ch;
					}
				}
			}
		}
		if (current) lines.push(current);
	}
	return lines.length > 0 ? lines : [""];
}
