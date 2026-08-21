import { formatBytes as formatBytesBase } from "@recast/editor/lib/format/bytes";

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

/** Byte count as a short human string. An absent cap renders as unlimited. */
export function formatBytes(bytes: number | null | undefined): string {
	return formatBytesBase(bytes, { zeroLabel: "0 GB", emptyLabel: "Unlimited" });
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
