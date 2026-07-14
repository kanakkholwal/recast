import { describe, expect, it } from "vitest";
import {
	formatClock,
	formatFrames,
	formatRulerTick,
	formatSmpte,
	formatTimeByMode,
} from "./time";

describe("formatSmpte", () => {
	it("drops the hours under an hour", () => {
		expect(formatSmpte(0, 60)).toBe("00:00:00");
		expect(formatSmpte(61.5, 60)).toBe("01:01:30");
	});

	it("keeps the hours past one", () => {
		expect(formatSmpte(3661, 30)).toBe("01:01:01:00");
	});

	it("never renders a negative time", () => {
		expect(formatSmpte(-5, 30)).toBe("00:00:00");
	});
});

describe("formatFrames", () => {
	it("reports the absolute frame index", () => {
		expect(formatFrames(2, 60)).toBe("120f");
		expect(formatFrames(0, 60)).toBe("0f");
	});
});

describe("formatTimeByMode", () => {
	it("routes each mode to its formatter", () => {
		expect(formatTimeByMode(1.5, "smpte", 60)).toBe(formatSmpte(1.5, 60));
		expect(formatTimeByMode(1.5, "seconds", 60)).toBe(formatClock(1.5));
		expect(formatTimeByMode(1.5, "frames", 60)).toBe(formatFrames(1.5, 60));
	});
});

describe("formatRulerTick", () => {
	// The bug: ticks 0.5s apart were all floored to a whole second, so a
	// zoomed-in ruler printed "0:00, 0:00, 0:01, 0:01".
	it("keeps sub-second ticks distinct", () => {
		const interval = 0.5;
		const labels = [0, 0.5, 1, 1.5, 2].map((t) =>
			formatRulerTick(t, "seconds", 60, interval),
		);
		expect(labels).toEqual(["0:00.0", "0:00.5", "0:01.0", "0:01.5", "0:02.0"]);
		expect(new Set(labels).size).toBe(labels.length);
	});

	it("stays compact at whole-second intervals", () => {
		expect(formatRulerTick(65, "seconds", 60, 5)).toBe("1:05");
	});

	// The other bug: the ruler ignored the Time display setting entirely.
	it("follows Frames mode", () => {
		expect(formatRulerTick(2, "frames", 60, 1)).toBe("120f");
	});
});
