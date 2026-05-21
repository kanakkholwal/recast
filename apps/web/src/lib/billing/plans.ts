import { serverEnv } from "$lib/env/server";

/**
 * Source of truth for plan limits. Server-side gates (analytics, watermark,
 * share count caps) read these. Polar product IDs are read lazily so the
 * module loads cleanly when env isn't set.
 */

export const PLAN_IDS = ["free", "pro"] as const;
export type PlanId = (typeof PLAN_IDS)[number];

export type PlanLimits = {
	activeShares: number; // Number.POSITIVE_INFINITY = no cap
	analytics: boolean;
	customBranding: boolean;
	passwordProtection: boolean;
	linkExpiry: boolean;
	watermark: boolean;
};

export type Plan = {
	id: PlanId;
	name: string;
	priceUsd: number;
	limits: PlanLimits;
};

export const PLANS: Record<PlanId, Plan> = {
	free: {
		id: "free",
		name: "Free",
		priceUsd: 0,
		limits: {
			activeShares: 10,
			analytics: false,
			customBranding: false,
			passwordProtection: false,
			linkExpiry: false,
			watermark: true,
		},
	},
	pro: {
		id: "pro",
		name: "Pro",
		priceUsd: 10,
		limits: {
			activeShares: Number.POSITIVE_INFINITY,
			analytics: true,
			customBranding: true,
			passwordProtection: true,
			linkExpiry: true,
			watermark: false,
		},
	},
};

/** Polar product ids — set in your Polar dashboard, then in .env. */
export function polarProductIdFor(plan: PlanId): string | null {
	switch (plan) {
		case "pro":
			return serverEnv().POLAR_PRODUCT_ID_PRO ?? null;
		case "free":
			return null;
	}
}
