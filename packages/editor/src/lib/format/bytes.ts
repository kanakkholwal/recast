const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

export type FormatBytesOptions = {
	/** Shown for 0 or negative input. */
	zeroLabel?: string;
	/** Shown for null/undefined/non-finite — e.g. an uncapped plan. Defaults to `zeroLabel`. */
	emptyLabel?: string;
};

/**
 * The one byte formatter: `1536` → `1.5 KB`, `191000000` → `182 MB`.
 * One decimal below 100, none at or above, so a usage meter and a file chip agree.
 */
export function formatBytes(
	bytes: number | null | undefined,
	opts: FormatBytesOptions = {},
): string {
	const { zeroLabel = "0 B", emptyLabel = zeroLabel } = opts;
	if (bytes == null || !Number.isFinite(bytes)) return emptyLabel;
	if (bytes <= 0) return zeroLabel;

	let value = bytes;
	let unit = 0;
	while (value >= 1024 && unit < UNITS.length - 1) {
		value /= 1024;
		unit += 1;
	}
	let digits = unit === 0 || value >= 100 ? 0 : 1;
	// toFixed can round 1023.7 MB up to "1024 MB"; promote the unit instead.
	if (Number(value.toFixed(digits)) >= 1024 && unit < UNITS.length - 1) {
		value /= 1024;
		unit += 1;
		digits = 1;
	}
	const text = value.toFixed(digits);
	return `${text.endsWith(".0") ? text.slice(0, -2) : text} ${UNITS[unit]}`;
}
