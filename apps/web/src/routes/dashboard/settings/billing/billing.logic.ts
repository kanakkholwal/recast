export type DeliveryView = {
	usedBytes: number;
	capBytes: number | null;
	ratio: number;
	exceeded: boolean;
	warn: boolean;
};

export type SeatView = {
	used: number;
	included: number;
	max: number;
	extraUsd: number;
	billable: number;
};

const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** Byte count as a short human string. `null` renders as unlimited. */
export function formatBytes(bytes: number | null | undefined): string {
	if (bytes == null || !Number.isFinite(bytes)) return "Unlimited";
	if (bytes <= 0) return "0 GB";
	let value = bytes;
	let unit = 0;
	while (value >= 1024 && unit < UNITS.length - 1) {
		value /= 1024;
		unit += 1;
	}
	return `${value >= 10 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${UNITS[unit]}`;
}

export function formatUsd(value: number | null | undefined): string {
	if (value == null) return "Custom";
	return Number.isInteger(value) ? `$${value}` : `$${value.toFixed(2)}`;
}

/**
 * Rough view count a delivery allowance buys, for copy like "≈600 views".
 * Assumes a 6-minute 1080p recast; deliberately conservative.
 */
const TYPICAL_RECAST_BYTES = 200 * 1024 * 1024;

export function approxViews(bytes: number | null | undefined): number | null {
	if (bytes == null || !Number.isFinite(bytes)) return null;
	return Math.round(bytes / TYPICAL_RECAST_BYTES);
}

export function seatView(
	membersCount: number,
	included: number,
	max: number,
	extraUsd: number,
): SeatView {
	return {
		used: membersCount,
		included,
		max,
		extraUsd,
		billable: Math.max(0, membersCount - included),
	};
}

/** Bar severity shared by the storage and delivery meters. */
export function meterTone(ratio: number): "neutral" | "warning" | "critical" {
	if (ratio >= 1) return "critical";
	if (ratio >= 0.8) return "warning";
	return "neutral";
}
