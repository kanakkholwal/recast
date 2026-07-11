/** Pure formatters for file listings (recordings, exports, activity strips). */

/** Human-readable byte size, e.g. `1.5 MB`. Caps at MB (recordings/exports). */
export function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / 1048576).toFixed(1)} MB`;
}

/** Upper-case file extension, e.g. `MP4`; `FILE` when there's no dot. */
export function getExtension(filename: string): string {
	const dot = filename.lastIndexOf(".");
	return dot >= 0 ? filename.slice(dot + 1).toUpperCase() : "FILE";
}

/** Extensions previewed as an image, not loaded into the video player. */
const IMAGE_EXTENSIONS = new Set([
	"gif",
	"webp",
	"apng",
	"png",
	"jpg",
	"jpeg",
	"avif",
]);

/**
 * True when a file should be previewed as an image (e.g. a GIF export) rather
 * than in the `<video>`-based player, which can't render a GIF.
 */
export function isImageFile(filename: string): boolean {
	const dot = filename.lastIndexOf(".");
	if (dot < 0) return false;
	return IMAGE_EXTENSIONS.has(filename.slice(dot + 1).toLowerCase());
}

/** Absolute short date, e.g. `Apr 10`. `unix` is epoch SECONDS. */
export function formatShortDate(unix: number): string {
	return new Date(unix * 1000).toLocaleDateString(undefined, {
		month: "short",
		day: "numeric",
	});
}

/** Absolute date + time, e.g. `Apr 10, 02:15 PM`. `unix` is epoch SECONDS. */
export function formatDateTime(unix: number): string {
	return new Date(unix * 1000).toLocaleDateString(undefined, {
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
}

/**
 * Relative age (`just now` / `5m ago` / `3d ago`), then an absolute date past a
 * week. `unix` is epoch SECONDS; `now` is the reference epoch in MILLISECONDS
 * (default `Date.now()`). `withTime` picks the >1-week fallback format.
 */
export function relativeDate(
	unix: number,
	opts: { now?: number; withTime?: boolean } = {},
): string {
	const now = opts.now ?? Date.now();
	const diff = now / 1000 - unix;
	if (diff < 60) return "just now";
	if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
	if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
	if (diff < 86400 * 7) return `${Math.floor(diff / 86400)}d ago`;
	return opts.withTime ? formatDateTime(unix) : formatShortDate(unix);
}
