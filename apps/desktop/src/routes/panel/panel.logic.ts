/** Pure mappers + fps/timer math for the recording panel window. */

import type { LastSource } from "$lib/ipc";

export type TargetSource = {
	type: "monitor" | "window" | "region";
	id: number;
	label: string;
	/** Monitor refresh rate in Hz (monitors only); caps the useful capture
	 *  fps so we never record above what the display can present. */
	refreshHz?: number;
	region?: {
		x: number;
		y: number;
		width: number;
		height: number;
	};
};

/**
 * Cap a desired capture fps to a monitor source's refresh rate. `null` (Auto)
 * and non-monitor / unknown-refresh sources pass through unchanged. The
 * backend still clamps to its 24–240 range.
 */
export function clampFpsToDisplay(
	desired: number | null,
	source: TargetSource | null,
): number | null {
	if (desired == null) return null;
	const cap = source?.type === "monitor" ? source.refreshHz : undefined;
	return cap && cap >= 1 ? Math.min(desired, cap) : desired;
}

/** Persisted `LastSource` → the panel's in-memory selected source. */
export function lastSourceToTarget(last: LastSource): TargetSource {
	return {
		type:
			last.kind === "window"
				? "window"
				: last.kind === "region"
					? "region"
					: "monitor",
		id: last.id,
		label: last.label,
		region:
			last.kind === "region" &&
			last.regionWidth != null &&
			last.regionHeight != null
				? {
						x: last.regionX ?? 0,
						y: last.regionY ?? 0,
						width: last.regionWidth,
						height: last.regionHeight,
					}
				: undefined,
	};
}

/** Selected source → the `LastSource` payload persisted for next launch. */
export function targetToLastSource(source: TargetSource): LastSource {
	return {
		kind:
			source.type === "monitor"
				? "monitor"
				: source.type === "window"
					? "window"
					: "region",
		id: source.id,
		label: source.label,
		regionX: source.region?.x ?? null,
		regionY: source.region?.y ?? null,
		regionWidth: source.region?.width ?? null,
		regionHeight: source.region?.height ?? null,
	};
}

/** Elapsed seconds → `MM:SS` (minutes zero-padded; recordings stay short). */
export function formatRecordingTimer(elapsedSeconds: number): string {
	const s = Math.max(0, Math.floor(elapsedSeconds));
	const mm = Math.floor(s / 60)
		.toString()
		.padStart(2, "0");
	const ss = (s % 60).toString().padStart(2, "0");
	return `${mm}:${ss}`;
}
