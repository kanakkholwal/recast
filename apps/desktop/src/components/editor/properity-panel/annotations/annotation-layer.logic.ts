/** AnnotationLayerPanel pure helpers. */

/**
 * Reorder the visual (top→bottom) id list by moving `dragId` into `targetId`'s
 * slot, returning the bottom→top z-order the store expects. `null` when either
 * id is missing (nothing to reorder).
 */
export function reorderZ(
	visualIds: string[],
	dragId: string,
	targetId: string,
): string[] | null {
	const fromIdx = visualIds.indexOf(dragId);
	const toIdx = visualIds.indexOf(targetId);
	if (fromIdx === -1 || toIdx === -1) return null;
	const next = [...visualIds];
	const [moved] = next.splice(fromIdx, 1);
	next.splice(toIdx, 0, moved);
	return [...next].reverse();
}
