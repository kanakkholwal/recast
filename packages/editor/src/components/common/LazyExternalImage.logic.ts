/** Pure src-tier selection + box sizing for LazyExternalImage. */

export type ImageTier = "thumb" | "full" | "auto";

/** Which cached URL to render for the requested tier; null when unavailable. */
export function pickSrc(
	tier: ImageTier,
	fullUrl: string | undefined,
	thumbUrl: string | undefined,
): string | null {
	if (tier === "thumb") return thumbUrl ?? null;
	if (tier === "full") return fullUrl ?? null;
	return fullUrl ?? thumbUrl ?? null;
}

/**
 * Reserve box space before any I/O so the skeleton → image swap never shifts
 * layout: explicit height wins, otherwise the aspect-ratio holds the box.
 */
export function boxStyle(width: string, height: string | undefined, aspectRatio: string): string {
	return [`width: ${width};`, height ? `height: ${height};` : `aspect-ratio: ${aspectRatio};`].join(
		" ",
	);
}
