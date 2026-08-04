/** AnnotationsPanel pure helpers: image basename + the active-tool status hint. */

import type { AnnotationKindName } from "../../stores/editor-store.svelte";

/** Last path segment, handling both `/` and `\` separators. */
export function imageFileName(path: string): string {
	const parts = path.split(/[/\\]/);
	return parts[parts.length - 1] || "Image";
}

/** Status hint shown beneath the palette while a tool is active. */
export function toolHint(tool: AnnotationKindName | null): string {
	switch (tool) {
		case "rect":
		case "ellipse":
			return "Drag on the preview to draw. Hold Shift for a square.";
		case "arrow":
			return "Drag from start to end. Hold Shift to snap to 45°.";
		case "text":
			return "Drag a box on the preview, then type.";
		case "blur":
			return "Drag a region to obscure. Applied at export.";
		default:
			return "";
	}
}
