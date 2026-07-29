import { describe, expect, it } from "vitest";
import {
	conflictingHotkeys,
	EngagementTracker,
	markerLeftPct,
	resolveDownloadPlan,
	volumeLevel,
} from "./player.logic";

describe("hotkey ownership", () => {
	it("never claims a key <media-controller> already handles", () => {
		expect(conflictingHotkeys()).toEqual([]);
	});
});

describe("markerLeftPct", () => {
	it("is 0 before metadata gives us a duration", () => {
		expect(markerLeftPct(12, 0)).toBe(0);
		expect(markerLeftPct(12, Number.NaN)).toBe(0);
		expect(markerLeftPct(12, Number.POSITIVE_INFINITY)).toBe(0);
	});

	it("clamps to the rail", () => {
		expect(markerLeftPct(-5, 100)).toBe(0);
		expect(markerLeftPct(250, 100)).toBe(100);
		expect(markerLeftPct(25, 100)).toBe(25);
	});
});

describe("volumeLevel", () => {
	it("maps the level ramp", () => {
		expect(volumeLevel(0.8, false)).toBe("high");
		expect(volumeLevel(0.5, false)).toBe("medium");
		expect(volumeLevel(0.2, false)).toBe("low");
		expect(volumeLevel(0, false)).toBe("muted");
		expect(volumeLevel(0.8, true)).toBe("muted");
	});
});

describe("resolveDownloadPlan", () => {
	it("uses the anchor download attribute for same-origin sources", () => {
		const plan = resolveDownloadPlan(
			"https://app.recast.li/media/clip.mp4",
			"Standup",
			"https://app.recast.li",
		);
		expect(plan).toEqual({ strategy: "anchor", filename: "Standup.mp4" });
	});

	it("falls back to a blob for cross-origin signed URLs", () => {
		const plan = resolveDownloadPlan(
			"https://bucket.r2.cloudflarestorage.com/x.mp4?X-Amz-Signature=abc",
			"Standup",
			"https://app.recast.li",
		);
		expect(plan.strategy).toBe("fetch-blob");
	});

	it("strips path separators from the title", () => {
		const plan = resolveDownloadPlan("/clip.mp4", "Q3/Q4 review", "https://app.recast.li");
		expect(plan.filename).not.toContain("/");
	});
});

describe("EngagementTracker", () => {
	it("reports view-start once per session", () => {
		const tracker = new EngagementTracker();
		expect(tracker.onPlay()).toEqual({ type: "view-start", percent: 0 });
		expect(tracker.onPlay()).toBeNull();
	});

	it("throttles progress to 5% steps", () => {
		const tracker = new EngagementTracker();
		expect(tracker.onTimeUpdate(2, 100)).toBeNull();
		expect(tracker.onTimeUpdate(5, 100)).toMatchObject({ percent: 5 });
		expect(tracker.onTimeUpdate(7, 100)).toBeNull();
		expect(tracker.onTimeUpdate(10, 100)).toMatchObject({ percent: 10 });
	});

	it("keeps reporting after the viewer scrubs backwards", () => {
		const tracker = new EngagementTracker();
		tracker.onTimeUpdate(80, 100);
		expect(tracker.onTimeUpdate(10, 100)).toMatchObject({ percent: 10 });
		expect(tracker.onTimeUpdate(15, 100)).toMatchObject({ percent: 15 });
	});

	it("starts a fresh session when the source changes", () => {
		const tracker = new EngagementTracker();
		tracker.onPlay();
		tracker.onTimeUpdate(50, 100);
		tracker.reset();
		expect(tracker.onPlay()).toEqual({ type: "view-start", percent: 0 });
		expect(tracker.onTimeUpdate(5, 100)).toMatchObject({ percent: 5 });
	});
});
