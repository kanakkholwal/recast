/** AnnotationGeometry pure helpers: frame-align maths. */

/**
 * Target UV position for aligning a box of size `box` to the frame along `axis`:
 * start = 0, end = flush against the far edge, center = centred.
 */
export function alignTarget(
	box: { w: number; h: number },
	axis: "x" | "y",
	anchor: "start" | "center" | "end",
): number {
	const extent = axis === "x" ? box.w : box.h;
	if (anchor === "start") return 0;
	if (anchor === "end") return 1 - extent;
	return 0.5 - extent / 2;
}
