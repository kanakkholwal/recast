import { describe, expect, it } from "vitest";
import { limitsFor, PLANS } from "$lib/billing/catalog";
import { currentDeliveryPeriodStart, deliveryState, type QuotaSnapshot } from "./quota.logic";

const GB = 1024 ** 3;

function snapshot(
	plan: "free" | "pro" | "enterprise",
	deliveryBytes: number,
	periodStart: Date,
): QuotaSnapshot {
	return {
		plan,
		limits: limitsFor(plan),
		usage: {
			storageBytes: 0,
			activeRecastsCount: 0,
			archivedRecastsCount: 0,
			membersCount: 1,
			deliveryBytesThisMonth: deliveryBytes,
			deliveryPeriodStart: periodStart,
		},
	} as QuotaSnapshot;
}

const JULY = new Date("2026-07-15T12:00:00Z");
const JULY_START = new Date(Date.UTC(2026, 6, 1));
const JUNE_START = new Date(Date.UTC(2026, 5, 1));

describe("currentDeliveryPeriodStart", () => {
	it("returns the first UTC instant of the month", () => {
		expect(currentDeliveryPeriodStart(JULY).toISOString()).toBe("2026-07-01T00:00:00.000Z");
	});

	// A local-time month boundary would shift the reset by hours for users
	// east of UTC, letting a workspace serve two "first days" in a row.
	it("uses UTC, not local time, at a month edge", () => {
		const edge = new Date("2026-08-01T00:30:00Z");
		expect(currentDeliveryPeriodStart(edge).toISOString()).toBe("2026-08-01T00:00:00.000Z");
	});
});

describe("deliveryState", () => {
	it("reports usage against the plan allowance", () => {
		const state = deliveryState(snapshot("free", 5 * GB, JULY_START), JULY);
		expect(state.usedBytes).toBe(5 * GB);
		expect(state.capBytes).toBe(25 * GB);
		expect(state.ratio).toBeCloseTo(0.2, 5);
		expect(state.exceeded).toBe(false);
		expect(state.warn).toBe(false);
	});

	it("warns at 80% of the allowance", () => {
		const state = deliveryState(snapshot("free", 20 * GB, JULY_START), JULY);
		expect(state.warn).toBe(true);
		expect(state.exceeded).toBe(false);
	});

	it("marks the cap exceeded exactly at the limit", () => {
		const state = deliveryState(snapshot("free", 25 * GB, JULY_START), JULY);
		expect(state.exceeded).toBe(true);
		expect(state.ratio).toBe(1);
	});

	// Without this, a workspace that maxed out in June would stay blocked
	// through July until something happened to write the counter.
	it("treats a previous month's counter as spent", () => {
		const state = deliveryState(snapshot("free", 99 * GB, JUNE_START), JULY);
		expect(state.usedBytes).toBe(0);
		expect(state.exceeded).toBe(false);
		expect(state.ratio).toBe(0);
	});

	// Enterprise is a contract, not an uncapped tier — it meters like the rest.
	it("meters enterprise against its contracted allowance", () => {
		const cap = PLANS.enterprise.limits.deliveryBytesPerMonth;
		const under = deliveryState(snapshot("enterprise", cap / 2, JULY_START), JULY);
		expect(under.exceeded).toBe(false);
		expect(under.ratio).toBeCloseTo(0.5, 5);

		const over = deliveryState(snapshot("enterprise", cap + GB, JULY_START), JULY);
		expect(over.exceeded).toBe(true);
	});

	it("clamps the ratio past the cap rather than exceeding 1", () => {
		const state = deliveryState(snapshot("free", 500 * GB, JULY_START), JULY);
		expect(state.ratio).toBe(1);
		expect(state.exceeded).toBe(true);
	});
});
