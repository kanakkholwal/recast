import { describe, expect, it } from "vitest";
import type { QuotaSnapshot } from "$lib/dashboard/store.svelte";
import { usageTone, usageView } from "./UsageMeter.logic";

const GB = 1024 ** 3;

function quota(overrides: Partial<QuotaSnapshot> = {}): QuotaSnapshot {
	return {
		plan: "free",
		usage: {
			storageBytes: GB,
			activeRecastsCount: 2,
			archivedRecastsCount: 0,
			membersCount: 1,
			deliveryBytesThisMonth: 5 * GB,
		},
		limits: {
			storageBytes: 5 * GB,
			activeRecasts: 10,
			members: 3,
			maxDurationSec: 600,
			playbackMaxHeight: 720,
			deliveryBytesPerMonth: 25 * GB,
		},
		storagePctUsed: 20,
		delivery: {
			usedBytes: 5 * GB,
			capBytes: 25 * GB,
			ratio: 0.2,
			exceeded: false,
			warn: false,
		},
		...overrides,
	} as QuotaSnapshot;
}

describe("usageTone", () => {
	it("stays neutral below the warning threshold", () => {
		expect(usageTone(0, true)).toBe("neutral");
		expect(usageTone(74, true)).toBe("neutral");
	});

	it("warns from 75% up to the critical threshold", () => {
		expect(usageTone(75, true)).toBe("warning");
		expect(usageTone(89, true)).toBe("warning");
	});

	it("goes critical from 90%", () => {
		expect(usageTone(90, true)).toBe("critical");
		expect(usageTone(100, true)).toBe("critical");
	});

	// The uncapped path now only happens when there is no quota loaded at all —
	// every plan carries a concrete ceiling.
	it("stays neutral at any percentage when no cap is known", () => {
		expect(usageTone(0, false)).toBe("neutral");
		expect(usageTone(95, false)).toBe("neutral");
		expect(usageTone(100, false)).toBe("neutral");
	});
});

describe("usageView delivery", () => {
	it("reports delivery against the monthly cap", () => {
		const view = usageView(quota());
		expect(view.deliveryBytes).toBe(5 * GB);
		expect(view.deliveryLimit).toBe(25 * GB);
		expect(view.deliveryPct).toBe(20);
		expect(view.deliveryStatus).toBe("80% left this month");
	});

	it("says shares are paused once the cap is hit", () => {
		const view = usageView(
			quota({
				delivery: {
					usedBytes: 25 * GB,
					capBytes: 25 * GB,
					ratio: 1,
					exceeded: true,
					warn: true,
				},
			}),
		);
		expect(view.deliveryPct).toBe(100);
		expect(view.deliveryStatus).toBe("Cap reached. Shares paused until the 1st");
		expect(view.deliveryTone).toBe("critical");
	});

	// Regression: quota snapshots persisted to localStorage before `delivery`
	// existed hydrate without it, and a non-optional read crashed the sidebar.
	it("survives a cached quota that predates the delivery field", () => {
		const stale = quota();
		delete (stale as Partial<QuotaSnapshot>).delivery;
		expect(() => usageView(stale)).not.toThrow();
		expect(usageView(stale).deliveryBytes).toBe(0);
		expect(usageView(stale).deliveryLimit).toBeNull();
	});

	it("handles a completely absent quota", () => {
		const view = usageView(null);
		expect(view.deliveryBytes).toBe(0);
		expect(view.deliveryPct).toBe(0);
	});
});
