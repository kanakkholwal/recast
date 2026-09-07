import { type PlanId, type QuotaLimits, USAGE_WARN_THRESHOLD } from "$lib/billing/catalog";

export type PlanKey = PlanId;

export type QuotaSnapshot = {
	plan: PlanKey;
	usage: {
		storageBytes: number;
		activeRecastsCount: number;
		archivedRecastsCount: number;
		membersCount: number;
		deliveryBytesThisMonth: number;
		deliveryPeriodStart: Date;
	};
	limits: QuotaLimits;
};

export type UploadDenial =
	| { reason: "workspace_not_found" }
	| { reason: "duration_over_cap"; capSec: number }
	| { reason: "resolution_over_cap"; heightPx: number; capHeight: number }
	| { reason: "active_recasts_over_cap"; current: number; cap: number }
	| {
			reason: "storage_over_cap";
			currentBytes: number;
			requestedBytes: number;
			capBytes: number;
	  };

// Encoders round to even dimensions, so allow slack: 720p landing at 722 isn't over 720p.
const RESOLUTION_SLACK_PX = 8;

/**
 * Pre-upload gate. Sizes are advisory here — `/api/uploads/complete` re-checks
 * the provider-reported size before committing the usage bump.
 */
export function checkUploadAllowed(
	snapshot: QuotaSnapshot,
	req: { sizeBytes: number; durationSec: number; heightPx?: number },
): { ok: true } | { ok: false; denial: UploadDenial } {
	const { limits, usage } = snapshot;

	if (req.durationSec > limits.maxDurationSec) {
		return {
			ok: false,
			denial: { reason: "duration_over_cap", capSec: limits.maxDurationSec },
		};
	}

	if (
		req.heightPx != null &&
		Number.isFinite(limits.playbackMaxHeight) &&
		req.heightPx > limits.playbackMaxHeight + RESOLUTION_SLACK_PX
	) {
		return {
			ok: false,
			denial: {
				reason: "resolution_over_cap",
				heightPx: req.heightPx,
				capHeight: limits.playbackMaxHeight,
			},
		};
	}

	if (usage.activeRecastsCount >= limits.activeRecasts) {
		return {
			ok: false,
			denial: {
				reason: "active_recasts_over_cap",
				current: usage.activeRecastsCount,
				cap: limits.activeRecasts,
			},
		};
	}

	const projected = usage.storageBytes + req.sizeBytes;
	if (projected > limits.storageBytes) {
		return {
			ok: false,
			denial: {
				reason: "storage_over_cap",
				currentBytes: usage.storageBytes,
				requestedBytes: req.sizeBytes,
				capBytes: limits.storageBytes,
			},
		};
	}

	return { ok: true };
}

/** % of the storage cap currently used. 0–100, clamped. */
export function storagePctUsed(snapshot: QuotaSnapshot): number {
	if (!Number.isFinite(snapshot.limits.storageBytes)) return 0;
	const pct = (snapshot.usage.storageBytes / snapshot.limits.storageBytes) * 100;
	return Math.min(100, Math.max(0, pct));
}

/** First instant of the current billing month. UTC so the reset is global. */
export function currentDeliveryPeriodStart(now = new Date()): Date {
	return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1));
}

export type DeliveryState = {
	usedBytes: number;
	capBytes: number;
	/** 0–1, clamped. Always 0 on unlimited plans. */
	ratio: number;
	exceeded: boolean;
	warn: boolean;
};

/**
 * Delivery is the dominant infra cost, so it gets its own gate. A counter from
 * a previous month reads as zero — the row resets lazily on the next write.
 */
export function deliveryState(snapshot: QuotaSnapshot, now = new Date()): DeliveryState {
	const capBytes = snapshot.limits.deliveryBytesPerMonth;
	const stale = snapshot.usage.deliveryPeriodStart < currentDeliveryPeriodStart(now);
	const usedBytes = stale ? 0 : snapshot.usage.deliveryBytesThisMonth;

	if (!Number.isFinite(capBytes)) {
		return { usedBytes, capBytes, ratio: 0, exceeded: false, warn: false };
	}
	const ratio = Math.min(1, Math.max(0, usedBytes / capBytes));
	return {
		usedBytes,
		capBytes,
		ratio,
		exceeded: usedBytes >= capBytes,
		warn: ratio >= USAGE_WARN_THRESHOLD,
	};
}
