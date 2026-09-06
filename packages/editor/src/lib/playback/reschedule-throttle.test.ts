import { describe, expect, it } from "vitest";
import { RESCHEDULE_MIN_INTERVAL_MS, rescheduleDecision } from "./reschedule-throttle";

describe("rescheduleDecision", () => {
	it("runs immediately when the last reschedule is old enough", () => {
		expect(rescheduleDecision(1000, 900, false)).toEqual({ act: "now" });
	});

	it("runs immediately on the very first request", () => {
		// lastRunMs of -Infinity is "never ran", which must not defer.
		expect(rescheduleDecision(0, Number.NEGATIVE_INFINITY, false)).toEqual({ act: "now" });
	});

	it("defers by exactly the remaining interval", () => {
		expect(rescheduleDecision(1020, 1000, false)).toEqual({ act: "defer", afterMs: 30 });
	});

	it("coalesces into the pending timer rather than stacking timers", () => {
		// A drag's many requests must collapse into one trailing run, not a timer each.
		expect(rescheduleDecision(1001, 1000, true)).toEqual({ act: "coalesce" });
		expect(rescheduleDecision(9999, 1000, true)).toEqual({ act: "coalesce" });
	});

	/**
	 * The bug this exists to prevent: a 125Hz mouse drag emitted ~125 full
	 * graph teardowns a second, each aborting the previous decode before it
	 * could make a sound.
	 */
	it("collapses a fast drag to the throttled rate", () => {
		let lastRun = Number.NEGATIVE_INFINITY;
		let firesAt: number | null = null;
		let runs = 0;
		const requests = [];
		for (let t = 1; t <= 500; t += 8) requests.push(t);

		for (const t of requests) {
			// The pending timer fires before any request that arrives after it.
			if (firesAt !== null && firesAt <= t) {
				runs += 1;
				lastRun = firesAt;
				firesAt = null;
			}
			const d = rescheduleDecision(t, lastRun, firesAt !== null);
			if (d.act === "now") {
				runs += 1;
				lastRun = t;
			} else if (d.act === "defer") {
				firesAt = t + d.afterMs;
			}
		}
		if (firesAt !== null) runs += 1;

		// 125Hz for 500ms is 63 requests; throttling must collapse them to ~10.
		expect(requests.length).toBe(63);
		expect(runs).toBeLessThanOrEqual(500 / RESCHEDULE_MIN_INTERVAL_MS + 1);
		expect(runs).toBeLessThan(requests.length / 4);
	});

	it("still runs a trailing reschedule so the final position lands", () => {
		// The last pointer move must not be swallowed, or sound stops short of the picture.
		const d = rescheduleDecision(1010, 1000, false);
		expect(d).toEqual({ act: "defer", afterMs: 40 });
	});

	it("honours a caller-supplied interval", () => {
		expect(rescheduleDecision(1010, 1000, false, 200)).toEqual({ act: "defer", afterMs: 190 });
	});

	it("matches the media source's seek interval", () => {
		// Video and audio chase a scrub at the same rate or they diverge on screen.
		expect(RESCHEDULE_MIN_INTERVAL_MS).toBe(50);
	});
});
