const GB = 1024 ** 3;

/**
 * Per-GB rates for the live storage provider. Every allowance below is sized
 * from these — swap the block when infra moves and re-run `pnpm test:plans`.
 */
export const INFRA = {
	provider: "azure",
	/** Azure Blob hot tier, first 50 TB (verified 2026-07-26). */
	storagePerGbMonth: 0.0184,
	/** Azure internet egress past the 100 GB/mo free allowance. */
	egressPerGb: 0.087,
} as const;

/**
 * Allowances to raise once egress is on R2 ($0/GB). Price does not change on
 * migration — only these go up, so users never see a downgrade.
 */
export const R2_TARGET_ALLOWANCES = {
	free: { storageBytes: 10 * GB, deliveryBytesPerMonth: 100 * GB },
	pro: { storageBytes: 250 * GB, deliveryBytesPerMonth: 1024 * GB },
} as const;

export const PLAN_IDS = ["free", "pro", "enterprise"] as const;
export type PlanId = (typeof PLAN_IDS)[number];

export type PlanSeats = {
	/** Creators covered by the base price. */
	included: number;
	/** Hard ceiling. Always a concrete number — no plan is uncapped. */
	max: number;
	monthlyUsd: number;
	annualMonthlyUsd: number;
};

export type PlanLimits = {
	storageBytes: number;
	/** Bytes served to viewers per calendar month — the real cost driver. */
	deliveryBytesPerMonth: number;
	activeRecasts: number;
	maxDurationSec: number;
	playbackMaxHeight: number;
	/** Archive after this many days with zero views; `null` = never. */
	expireAfterNoViewsDays: number | null;
	hardDeleteAfterArchiveDays: number | null;
};

export type PlanFeatures = {
	analytics: boolean;
	customBranding: boolean;
	passwordProtection: boolean;
	linkExpiry: boolean;
	perViewerAccess: boolean;
	auditLog: boolean;
	sso: boolean;
};

export type Plan = {
	id: PlanId;
	name: string;
	/** `null` = contact-sales, no self-serve checkout. */
	monthlyUsd: number | null;
	annualMonthlyUsd: number | null;
	seats: PlanSeats;
	limits: PlanLimits;
	features: PlanFeatures;
};

/**
 * Single source of truth for every plan number in the product. Storage and
 * delivery are sized so Pro lands near break-even at `INFRA` rates.
 */
export const PLANS: Record<PlanId, Plan> = {
	free: {
		id: "free",
		name: "Free",
		monthlyUsd: 0,
		annualMonthlyUsd: 0,
		seats: { included: 3, max: 3, monthlyUsd: 0, annualMonthlyUsd: 0 },
		limits: {
			storageBytes: 5 * GB,
			deliveryBytesPerMonth: 25 * GB,
			activeRecasts: 10,
			maxDurationSec: 600,
			playbackMaxHeight: 720,
			expireAfterNoViewsDays: 14,
			hardDeleteAfterArchiveDays: 16,
		},
		features: {
			analytics: false,
			customBranding: false,
			passwordProtection: false,
			linkExpiry: false,
			perViewerAccess: false,
			auditLog: false,
			sso: false,
		},
	},
	pro: {
		id: "pro",
		name: "Pro",
		monthlyUsd: 12,
		annualMonthlyUsd: 10,
		seats: { included: 3, max: 50, monthlyUsd: 4, annualMonthlyUsd: 3.3 },
		limits: {
			storageBytes: 50 * GB,
			deliveryBytesPerMonth: 125 * GB,
			activeRecasts: 200,
			maxDurationSec: 4 * 60 * 60,
			playbackMaxHeight: 2160,
			expireAfterNoViewsDays: null,
			hardDeleteAfterArchiveDays: null,
		},
		features: {
			analytics: true,
			customBranding: true,
			passwordProtection: true,
			linkExpiry: true,
			perViewerAccess: true,
			auditLog: false,
			sso: false,
		},
	},
	// Starting point for a contract, not a live entitlement — every number here
	// is overridden per agreement via the workspace's `*Limit` columns.
	enterprise: {
		id: "enterprise",
		name: "Enterprise",
		monthlyUsd: null,
		annualMonthlyUsd: null,
		seats: { included: 25, max: 250, monthlyUsd: 0, annualMonthlyUsd: 0 },
		limits: {
			storageBytes: 2048 * GB,
			deliveryBytesPerMonth: 10240 * GB,
			activeRecasts: 5000,
			maxDurationSec: 8 * 60 * 60,
			playbackMaxHeight: 2160,
			expireAfterNoViewsDays: null,
			hardDeleteAfterArchiveDays: null,
		},
		features: {
			analytics: true,
			customBranding: true,
			passwordProtection: true,
			linkExpiry: true,
			perViewerAccess: true,
			auditLog: true,
			sso: true,
		},
	},
};

export type QuotaLimits = {
	storageBytes: number;
	deliveryBytesPerMonth: number;
	activeRecasts: number;
	maxDurationSec: number;
	members: number;
	playbackMaxHeight: number;
	expireAfterNoViewsDays: number | null;
	hardDeleteAfterArchiveDays: number | null;
};

/**
 * Negotiated per-workspace caps. Set on Enterprise contracts; `null` on any
 * field falls back to the plan's number.
 */
export type WorkspaceLimitOverrides = {
	seatLimit?: number | null;
	storageLimitBytes?: number | null;
	deliveryLimitBytes?: number | null;
	activeRecastsLimit?: number | null;
};

/** Flattens a plan into the shape the enforcement paths read. */
export function limitsFor(planId: PlanId, overrides?: WorkspaceLimitOverrides | null): QuotaLimits {
	const plan = PLANS[planId];
	const pick = (override: number | null | undefined, fallback: number): number =>
		override != null && override > 0 ? override : fallback;

	return {
		storageBytes: pick(overrides?.storageLimitBytes, plan.limits.storageBytes),
		deliveryBytesPerMonth: pick(overrides?.deliveryLimitBytes, plan.limits.deliveryBytesPerMonth),
		activeRecasts: pick(overrides?.activeRecastsLimit, plan.limits.activeRecasts),
		maxDurationSec: plan.limits.maxDurationSec,
		members: pick(overrides?.seatLimit, plan.seats.max),
		playbackMaxHeight: plan.limits.playbackMaxHeight,
		expireAfterNoViewsDays: plan.limits.expireAfterNoViewsDays,
		hardDeleteAfterArchiveDays: plan.limits.hardDeleteAfterArchiveDays,
	};
}

/** Charged past the included delivery allowance. ~38% over `INFRA.egressPerGb`. */
export const DELIVERY_OVERAGE_USD_PER_GB = 0.12;

/** Warn the workspace owner once usage crosses this share of any allowance. */
export const USAGE_WARN_THRESHOLD = 0.8;

export function isPlanId(value: unknown): value is PlanId {
	return PLAN_IDS.includes(value as PlanId);
}

/** Unknown/legacy plan strings resolve to free rather than throwing. */
export function planOf(value: string | null | undefined): Plan {
	return isPlanId(value) ? PLANS[value] : PLANS.free;
}

/** Paid tiers share every entitlement gate — never compare against "pro" alone. */
export function isPaidPlan(value: string | null | undefined): boolean {
	return planOf(value).id !== "free";
}

/** Monthly cost of `seats` creators, base price included. */
export function priceForSeats(planId: PlanId, seats: number, annual = false): number | null {
	const plan = PLANS[planId];
	const base = annual ? plan.annualMonthlyUsd : plan.monthlyUsd;
	if (base === null) return null;
	const extra = Math.max(0, seats - plan.seats.included);
	const perSeat = annual ? plan.seats.annualMonthlyUsd : plan.seats.monthlyUsd;
	return Number((base + extra * perSeat).toFixed(2));
}

/** What a workspace costs us per month at `INFRA` rates, for margin checks. */
export function infraCostUsd(storageBytes: number, deliveryBytes: number): number {
	const storage = (storageBytes / GB) * INFRA.storagePerGbMonth;
	const delivery = (deliveryBytes / GB) * INFRA.egressPerGb;
	return Number((storage + delivery).toFixed(4));
}
