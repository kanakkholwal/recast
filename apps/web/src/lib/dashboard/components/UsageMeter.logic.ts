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
	activeRecasts: number;
	linksLimit: number | null;
	linksPct: number;
	planLabel: string;
	storageStatus: string;
	linksStatus: string;
}

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

	return {
		usedBytes,
		storageLimit,
		storagePct,
		activeRecasts,
		linksLimit,
		linksPct,
		planLabel: planLabelOf(quota?.plan),
		storageStatus,
		linksStatus,
	};
}
