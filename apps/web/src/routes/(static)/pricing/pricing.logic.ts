import { PLANS, priceForSeats } from "$lib/billing/catalog";

/** Loom's published per-creator pricing, for the side-by-side. Verified 2026-07-26. */
export const LOOM = { monthlyUsd: 18, annualMonthlyUsd: 15 } as const;

export type TeamRow = {
	label: string;
	seats: number;
	recast: number;
	loom: number;
	savingPct: number;
};

const TEAM_SIZES: { label: string; seats: number }[] = [
	{ label: "Solo founder", seats: 1 },
	{ label: "Small team", seats: 5 },
	{ label: "Growing company", seats: 20 },
];

/** What each team size pays us vs Loom, at the same billing cadence. */
export function teamComparison(annual: boolean): TeamRow[] {
	const loomRate = annual ? LOOM.annualMonthlyUsd : LOOM.monthlyUsd;
	return TEAM_SIZES.map(({ label, seats }) => {
		const recast = priceForSeats("pro", seats, annual) ?? 0;
		const loom = seats * loomRate;
		return {
			label,
			seats,
			recast,
			loom,
			savingPct: Math.round(((loom - recast) / loom) * 100),
		};
	});
}

export function proPrice(annual: boolean): number {
	return (annual ? PLANS.pro.annualMonthlyUsd : PLANS.pro.monthlyUsd) ?? 0;
}

export function extraSeatPrice(annual: boolean): number {
	return annual ? PLANS.pro.seats.annualMonthlyUsd : PLANS.pro.seats.monthlyUsd;
}

export function formatUsd(value: number): string {
	return Number.isInteger(value) ? `$${value}` : `$${value.toFixed(2)}`;
}

const GB = 1024 ** 3;

export function gb(bytes: number): string {
	return Number.isFinite(bytes) ? `${Math.round(bytes / GB)} GB` : "Unlimited";
}
