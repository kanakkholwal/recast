/** AnnotationAppearance pure helpers. */

import type { AnnotationGlow } from "$lib/stores/editor-store.svelte";

/**
 * Glow base used when enabling glow or patching a field: keep the existing glow
 * if present, otherwise seed from the stroke colour (falling back to blue).
 */
export function defaultGlow(
	existing: AnnotationGlow | undefined,
	strokeColor: string,
): AnnotationGlow {
	return (
		existing ?? {
			color: strokeColor || "#3b82f6",
			blur: 0.012,
			opacity: 0.7,
		}
	);
}
