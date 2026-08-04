import type { IconComponent } from "@recast/icons";
import { MousePointer2 } from "@recast/icons";
import type { AnnotationKindName } from "../../stores/editor-store.svelte";
import { defaultKindIcon, defaultKindLabel } from "./kind-label";

export type AnnotationToolId = AnnotationKindName | "select";

export interface AnnotationTool {
	id: AnnotationToolId;
	label: string;
	icon: IconComponent;
	/** Single-key shortcut, uppercase for display. */
	hotkey: string;
}

function tool(id: AnnotationKindName, hotkey: string): AnnotationTool {
	return { id, label: defaultKindLabel(id), icon: defaultKindIcon(id), hotkey };
}

/**
 * Modal drawing tools in palette order, shared by the canvas toolbar and the
 * markup panel so the two can't drift on labels, icons or shortcuts.
 *
 * `select` is the null tool (no mode armed). `image` is deliberately absent: it
 * is a one-shot insert via the file picker, so its tile could never light up.
 */
export const ANNOTATION_TOOLS: AnnotationTool[] = [
	{ id: "select", label: "Select", icon: MousePointer2, hotkey: "V" },
	tool("rect", "R"),
	tool("ellipse", "O"),
	tool("arrow", "A"),
	tool("text", "T"),
	tool("blur", "B"),
];

export const IMAGE_TOOL = {
	label: defaultKindLabel("image"),
	icon: defaultKindIcon("image"),
	hotkey: "I",
} as const;

/** The tool a single-key press selects, or null when the key isn't a shortcut. */
export function toolForHotkey(key: string): AnnotationTool | null {
	const k = key.toLowerCase();
	return ANNOTATION_TOOLS.find((t) => t.hotkey.toLowerCase() === k) ?? null;
}
