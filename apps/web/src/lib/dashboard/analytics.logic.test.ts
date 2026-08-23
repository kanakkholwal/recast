import { describe, expect, it } from "vitest";
import type { Activity } from "./activity";
import {
	deltaPct,
	dropOffPoint,
	inRange,
	peakBucket,
	previousRange,
	returningViewers,
} from "./analytics.logic";

const DAY = 86_400_000;
const NOW = 1_700_000_000_000;

function view(daysAgo: number, session: string, watchPct = 50): Activity {
	return {
		id: `${session}-${daysAgo}`,
		recastId: "r_1",
		recastTitle: "Demo",
		viewer: "Anonymous viewer",
		sessionId: session,
		kind: "viewed",
		timestamp: NOW - daysAgo * DAY,
		watchPct,
	};
}

describe("inRange", () => {
	const activity = [view(1, "a"), view(10, "b"), view(200, "c")];

	it("keeps only the selected window", () => {
		expect(inRange(activity, "7d", NOW)).toHaveLength(1);
		expect(inRange(activity, "30d", NOW)).toHaveLength(2);
	});

	it("treats all-time as unbounded, not as a year", () => {
		expect(inRange(activity, "all", NOW)).toHaveLength(3);
	});
});

describe("previousRange", () => {
	it("is the equal-length window immediately before", () => {
		const activity = [view(3, "a"), view(9, "b"), view(20, "c")];
		const prev = previousRange(activity, "7d", NOW);
		expect(prev.map((a) => a.sessionId)).toEqual(["b"]);
	});

	it("is empty for all-time, which has no baseline", () => {
		expect(previousRange([view(3, "a")], "all", NOW)).toEqual([]);
	});
});

describe("deltaPct", () => {
	it("computes the change against the prior period", () => {
		expect(deltaPct(150, 100)).toBe(50);
		expect(deltaPct(50, 100)).toBe(-50);
	});

	it("returns null with no baseline, rather than a meaningless +100%", () => {
		expect(deltaPct(5, 0)).toBeNull();
	});
});

describe("returningViewers", () => {
	it("counts a viewer who came back on another day", () => {
		const activity = [view(1, "a"), view(3, "a"), view(1, "b")];
		expect(returningViewers(activity)).toEqual({ count: 1, pct: 50 });
	});

	it("does not count two views on the same day as returning", () => {
		const activity = [view(1, "a"), view(1, "a")];
		expect(returningViewers(activity).count).toBe(0);
	});

	it("is zero on an empty list", () => {
		expect(returningViewers([])).toEqual({ count: 0, pct: 0 });
	});
});

describe("dropOffPoint", () => {
	it("finds where the curve first falls below half", () => {
		const curve = [
			{ pct: 10, reached: 90 },
			{ pct: 20, reached: 70 },
			{ pct: 30, reached: 45 },
			{ pct: 40, reached: 20 },
		];
		expect(dropOffPoint(curve)).toBe(30);
	});

	it("is null when most viewers finish", () => {
		expect(dropOffPoint([{ pct: 100, reached: 80 }])).toBeNull();
	});
});

describe("peakBucket", () => {
	it("picks the busiest bucket", () => {
		const b = peakBucket([
			{ label: "Mon", views: 2 },
			{ label: "Tue", views: 9 },
			{ label: "Wed", views: 4 },
		]);
		expect(b?.label).toBe("Tue");
	});

	it("is null when nothing happened", () => {
		expect(peakBucket([{ label: "Mon", views: 0 }])).toBeNull();
	});
});
