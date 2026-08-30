import { domToBlob } from "modern-screenshot";
import type { ExportSpec } from "./types";

/** Editing affordances that live inside the stage but must never be baked into
 * the output (rulers, grid, selection handles). Mark such nodes with
 * `data-export-ignore` and every snapshot path drops them. */
export const EXPORT_IGNORE_ATTR = "data-export-ignore";

/** modern-screenshot node filter: keep everything except ignored guide nodes. */
export function exportFilter(node: Node): boolean {
	return !(node instanceof Element && node.hasAttribute(EXPORT_IGNORE_ATTR));
}

/** Snapshot the stage node to a Blob. Because the stage is the real DOM tree
 * the user edits, the export is pixel-identical to the preview; `scale` raises
 * the device-pixel ratio for crisp high-res output. */
const MIME: Record<ExportSpec["format"], string> = {
	png: "image/png",
	jpeg: "image/jpeg",
	webp: "image/webp",
};

export async function snapshot(node: HTMLElement, spec: ExportSpec): Promise<Blob> {
	const lossy = spec.format === "jpeg" || spec.format === "webp";
	const blob = await domToBlob(node, {
		type: MIME[spec.format],
		quality: lossy ? (spec.quality ?? 0.95) : undefined,
		scale: spec.scale,
		filter: exportFilter,
		// JPEG has no alpha, so give it an opaque backing or a transparent stage turns black; WebP and PNG keep theirs.
		backgroundColor: spec.format === "jpeg" ? "#ffffff" : undefined,
	});
	if (!blob) throw new Error("export failed to produce an image");
	return blob;
}

/** Trigger a browser download of `blob` as `filename`. */
export function download(blob: Blob, filename: string): void {
	const url = URL.createObjectURL(blob);
	const a = document.createElement("a");
	a.href = url;
	a.download = filename;
	document.body.appendChild(a);
	a.click();
	a.remove();
	// Revoke after the click has had a tick to start the download.
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** True when the clipboard can accept images (Firefox and some webviews can't). */
export function canCopyImage(): boolean {
	return (
		typeof ClipboardItem !== "undefined" &&
		typeof navigator !== "undefined" &&
		!!navigator.clipboard?.write
	);
}

/** Copy a PNG snapshot of the stage to the system clipboard. */
export async function copyToClipboard(node: HTMLElement, scale: number): Promise<void> {
	if (!canCopyImage()) throw new Error("this browser can't copy images to the clipboard");
	const blob = await snapshot(node, { format: "png", scale });
	await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
}

const EXT: Record<ExportSpec["format"], string> = { png: "png", jpeg: "jpg", webp: "webp" };

/** A timestamp-free, human-readable default filename. */
export function defaultFilename(format: ExportSpec["format"], base = "screenshot"): string {
	return `${base}.${EXT[format]}`;
}
