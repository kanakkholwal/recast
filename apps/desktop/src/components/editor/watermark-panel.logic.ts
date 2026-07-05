/** Pure helpers for WatermarkPanel. */

/** Basename of a file path (handles both `/` and `\` separators). */
export function getFileLabel(path: string): string {
	const segments = path.split(/[/\\]/);
	return segments[segments.length - 1] ?? "Selected image";
}
