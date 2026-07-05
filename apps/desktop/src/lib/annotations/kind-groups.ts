/** Annotation kind-group predicates shared across the property panels. */

import type {
	AnnotationKind,
	AnnotationKindName,
} from "$lib/stores/editor-store.svelte";

/** Vector shapes drawn with a stroke: rect, ellipse, arrow. */
export function isShape(kind: AnnotationKindName): boolean {
	return kind === "rect" || kind === "ellipse" || kind === "arrow";
}

/** Kinds framed by a stroke — shapes plus images (which get a border). */
export function hasStroke(kind: AnnotationKindName): boolean {
	return isShape(kind) || kind === "image";
}

/** Only rect and ellipse take a fill. */
export function hasFill(kind: AnnotationKindName): boolean {
	return kind === "rect" || kind === "ellipse";
}

/** Box-model kinds positioned by an (x, y, w, h) rect — everything but arrow. */
export function isBoxKind(
	kind: AnnotationKind,
): kind is Extract<AnnotationKind, { w: number }> {
	return (
		kind.kind === "rect" ||
		kind.kind === "ellipse" ||
		kind.kind === "text" ||
		kind.kind === "image" ||
		kind.kind === "blur"
	);
}
