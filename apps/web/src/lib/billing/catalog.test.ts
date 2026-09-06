import { describe, expect, it } from "vitest";
import {
	DELIVERY_OVERAGE_USD_PER_GB,
	INFRA,
	infraCostUsd,
	isPaidPlan,
	limitsFor,
	PLAN_IDS,
	PLANS,
	planOf,
	priceForSeats,
} from "./catalog";

const GB = 1024 ** 3;

describe("plan resolution", () => {
	it("resolves unknown and legacy plan strings to free", () => {
		expect(planOf(undefined).id).toBe("free");
		expect(planOf(null).id).toBe("free");
		expect(planOf("team").id).toBe("free");
	});

	// Regression: comparing only against 'pro' silently demoted Enterprise workspaces to Free and forced link expiry.
	it("treats enterprise as paid", () => {
		expect(isPaidPlan("enterprise")).toBe(true);
		expect(isPaidPlan("pro")).toBe(true);
		expect(isPaidPlan("free")).toBe(false);
	});

	it("exposes enterprise as a real plan id", () => {
		expect(PLANS.enterprise.id).toBe("enterprise");
		expect(PLANS.enterprise.monthlyUsd).toBeNull();
	});
});

describe("seat pricing", () => {
	it("charges the base price up to the included seats", () => {
		expect(priceForSeats("pro", 1)).toBe(12);
		expect(priceForSeats("pro", 3)).toBe(12);
	});

	it("adds per-seat cost past the included count", () => {
		expect(priceForSeats("pro", 4)).toBe(16);
		expect(priceForSeats("pro", 5)).toBe(20);
		expect(priceForSeats("pro", 20)).toBe(80);
	});

	it("discounts annual billing", () => {
		expect(priceForSeats("pro", 1, true)).toBe(10);
		expect(priceForSeats("pro", 5, true)).toBe(16.6);
	});

	it("returns null for contact-sales plans", () => {
		expect(priceForSeats("enterprise", 50)).toBeNull();
	});

	it("never charges below the base price for a partial team", () => {
		expect(priceForSeats("pro", 0)).toBe(12);
	});
});

describe("undercutting Loom", () => {
	const LOOM_MONTHLY = 18;
	const LOOM_ANNUAL = 15;

	it("beats Loom by at least 20% for a solo founder on both cadences", () => {
		const monthly = priceForSeats("pro", 1)!;
		const annual = priceForSeats("pro", 1, true)!;
		expect((LOOM_MONTHLY - monthly) / LOOM_MONTHLY).toBeGreaterThanOrEqual(0.2);
		expect((LOOM_ANNUAL - annual) / LOOM_ANNUAL).toBeGreaterThanOrEqual(0.2);
	});

	it("widens the gap as the team grows", () => {
		const solo = 1 - priceForSeats("pro", 1)! / LOOM_MONTHLY;
		const team = 1 - priceForSeats("pro", 5)! / (LOOM_MONTHLY * 5);
		expect(team).toBeGreaterThan(solo);
	});
});

describe("infra cost basis", () => {
	it("prices egress above storage — delivery is the real cost driver", () => {
		expect(INFRA.egressPerGb).toBeGreaterThan(INFRA.storagePerGbMonth);
	});

	it("bills delivery overage above what it costs us", () => {
		expect(DELIVERY_OVERAGE_USD_PER_GB).toBeGreaterThan(INFRA.egressPerGb);
	});

	// The whole point of the Azure-sized allowances: Pro must not be structurally underwater at list rates.
	it("keeps a fully-used Pro workspace at or under its base price", () => {
		const cost = infraCostUsd(
			PLANS.pro.limits.storageBytes,
			PLANS.pro.limits.deliveryBytesPerMonth,
		);
		expect(cost).toBeLessThanOrEqual(PLANS.pro.monthlyUsd!);
	});

	it("keeps a fully-used free workspace under the cost of one Pro seat", () => {
		const cost = infraCostUsd(
			PLANS.free.limits.storageBytes,
			PLANS.free.limits.deliveryBytesPerMonth,
		);
		expect(cost).toBeLessThan(PLANS.pro.seats.monthlyUsd);
	});

	it("computes cost from both dimensions", () => {
		expect(infraCostUsd(GB, GB)).toBeCloseTo(INFRA.storagePerGbMonth + INFRA.egressPerGb, 4);
	});
});

describe("negotiated workspace overrides", () => {
	it("falls back to the plan template when nothing is negotiated", () => {
		const limits = limitsFor("enterprise", null);
		expect(limits.members).toBe(PLANS.enterprise.seats.max);
		expect(limits.storageBytes).toBe(PLANS.enterprise.limits.storageBytes);
	});

	it("applies a contracted seat and storage cap", () => {
		const limits = limitsFor("enterprise", {
			seatLimit: 400,
			storageLimitBytes: 5000 * GB,
		});
		expect(limits.members).toBe(400);
		expect(limits.storageBytes).toBe(5000 * GB);
		// Untouched fields still come from the template.
		expect(limits.activeRecasts).toBe(PLANS.enterprise.limits.activeRecasts);
	});

	it("lets a contract cap BELOW the template, not just above", () => {
		expect(limitsFor("enterprise", { seatLimit: 30 }).members).toBe(30);
	});

	// A zero or negative override would silently disable the workspace.
	it("ignores non-positive overrides", () => {
		expect(limitsFor("pro", { seatLimit: 0 }).members).toBe(PLANS.pro.seats.max);
		expect(limitsFor("pro", { seatLimit: -5 }).members).toBe(PLANS.pro.seats.max);
	});

	it("keeps overrides finite", () => {
		const limits = limitsFor("enterprise", { seatLimit: 1000 });
		expect(Number.isFinite(limits.members)).toBe(true);
	});
});

describe("plan ladder", () => {
	it("never loosens a limit as the tier drops", () => {
		expect(PLANS.pro.limits.storageBytes).toBeGreaterThan(PLANS.free.limits.storageBytes);
		expect(PLANS.pro.limits.deliveryBytesPerMonth).toBeGreaterThan(
			PLANS.free.limits.deliveryBytesPerMonth,
		);
		expect(PLANS.pro.limits.activeRecasts).toBeGreaterThan(PLANS.free.limits.activeRecasts);
		expect(PLANS.enterprise.limits.storageBytes).toBeGreaterThan(PLANS.pro.limits.storageBytes);
		expect(PLANS.enterprise.seats.max).toBeGreaterThan(PLANS.pro.seats.max);
	});

	// No tier is uncapped: an Infinity limit can't be metered or invoiced, and it disables every enforcement check.
	it("gives every plan a concrete, finite ceiling", () => {
		for (const id of PLAN_IDS) {
			const { limits, seats } = PLANS[id];
			expect(Number.isFinite(seats.max)).toBe(true);
			expect(Number.isFinite(limits.storageBytes)).toBe(true);
			expect(Number.isFinite(limits.deliveryBytesPerMonth)).toBe(true);
			expect(Number.isFinite(limits.activeRecasts)).toBe(true);
			expect(Number.isFinite(limits.maxDurationSec)).toBe(true);
		}
	});
});
