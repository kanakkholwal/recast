/**
 * Derives the UsageMeter view-model from a plain quota snapshot: bar percentages,
 * plan label, and the free-space / limit-reached status strings. Unlimited caps
 * (null limit, Enterprise) collapse to 0% bars and an "Unlimited" status.
 */

import type { QuotaSnapshot } from "$lib/dashboard/store.svelte";

export interface UsageView {
	usedBytes: number;
	storageLimit: number | null;
	storagePct: number;
	storageTone: UsageTone;
	activeRecasts: number;
	linksLimit: number | null;
	linksPct: number;
	linksTone: UsageTone;
	deliveryBytes: number;
	deliveryLimit: number | null;
	deliveryPct: number;
	deliveryTone: UsageTone;
	deliveryStatus: string;
	planLabel: string;
	storageStatus: string;
	linksStatus: string;
}

/** Severity of a usage bar. Uncapped plans never warn, however high the usage. */
export type UsageTone = "neutral" | "warning" | "critical";

export const usageTone = (pct: number, capped: boolean): UsageTone =>
	!capped ? "neutral" : pct >= 90 ? "critical" : pct >= 75 ? "warning" : "neutral";

const planLabelOf = (plan: QuotaSnapshot["plan"] | undefined): string =>
	plan === "pro" ? "Pro" : plan === "enterprise" ? "Enterprise" : "Free";

export function usageView(quota: QuotaSnapshot | null): UsageView {
	const usedBytes = quota?.usage.storageBytes ?? 0;
	const storageLimit = quota?.limits.storageBytes ?? null;
	const storagePct = Math.round(quota?.storagePctUsed ?? 0);

	const activeRecasts = quota?.usage.activeRecastsCount ?? 0;
	const linksLimit = quota?.limits.activeRecasts ?? null;
	const linksPct =
		linksLimit && linksLimit > 0
			? Math.min(100, Math.round((activeRecasts / linksLimit) * 100))
			: 0;

	const storageStatus =
		storageLimit == null
			? "Unlimited"
			: storagePct >= 100
				? "Cap reached. Archive or upgrade"
				: `${100 - storagePct}% free`;

	const linksStatus =
		linksLimit == null
			? "Unlimited"
			: linksPct >= 100
				? "Limit reached"
				: `${linksLimit - activeRecasts} remaining`;

	// Optional-chained through `delivery` itself: a quota cached in localStorage
	// before this field existed would otherwise throw on hydrate.
	const deliveryBytes = quota?.delivery?.usedBytes ?? 0;
	const deliveryLimit = quota?.delivery?.capBytes ?? null;
	const deliveryPct = Math.min(100, Math.round((quota?.delivery?.ratio ?? 0) * 100));
	const deliveryStatus =
		deliveryLimit == null
			? "—"
			: quota?.delivery?.exceeded
				? "Cap reached. Shares paused until the 1st"
				: `${100 - deliveryPct}% left this month`;

	return {
		usedBytes,
		storageLimit,
		storagePct,
		storageTone: usageTone(storagePct, storageLimit != null),
		activeRecasts,
		linksLimit,
		linksPct,
		linksTone: usageTone(linksPct, linksLimit != null),
		deliveryBytes,
		deliveryLimit,
		deliveryPct,
		deliveryTone: usageTone(deliveryPct, deliveryLimit != null),
		deliveryStatus,
		planLabel: planLabelOf(quota?.plan),
		storageStatus,
		linksStatus,
	};
}
