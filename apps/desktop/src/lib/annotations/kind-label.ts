// Display helpers for annotations — kind-aware label and icon. Used by the
// layer panel, the status rail, and the selected-annotation header.

import type { Annotation, AnnotationKindName } from "$lib/stores/editor-store.svelte";
import {
	ArrowUpRight,
	Circle,
	Droplets,
	ImageIcon,
	Square,
	Type as TypeIcon,
} from "@lucide/svelte";

export function kindLabel(a: Annotation): string {
	if (a.name && a.name.trim()) return a.name.trim();
	switch (a.kind.kind) {
		case "rect":
			return "Rectangle";
		case "ellipse":
			return "Ellipse";
		case "arrow":
			return "Arrow";
		case "text":
			return a.kind.content.trim().slice(0, 32) || "Text";
		case "image":
			return "Image";
		case "blur":
			return "Blur";
	}
}

export function defaultKindLabel(kind: AnnotationKindName): string {
	switch (kind) {
		case "rect":
			return "Rectangle";
		case "ellipse":
			return "Ellipse";
		case "arrow":
			return "Arrow";
		case "text":
			return "Text";
		case "image":
			return "Image";
		case "blur":
			return "Blur";
	}
}

export function kindIcon(a: Annotation): typeof Square {
	switch (a.kind.kind) {
		case "rect":
			return Square;
		case "ellipse":
			return Circle;
		case "arrow":
			return ArrowUpRight;
		case "text":
			return TypeIcon;
		case "image":
			return ImageIcon;
		case "blur":
			return Droplets;
	}
}
