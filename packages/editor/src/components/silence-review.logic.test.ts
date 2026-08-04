import { describe, expect, it } from "vitest";
import {
	confidenceBarClass,
	confidenceLabel,
	confidenceTextClass,
	cutBounds,
	parseSensitivity,
} from "./silence-review.logic";

describe("parseSensitivity", () => {
	it("accepts the non-default presets and falls back to balanced", () => {
		expect(parseSensitivity("relaxed")).toBe("relaxed");
		expect(parseSensitivity("aggressive")).toBe("aggressive");
		expect(parseSensitivity("balanced")).toBe("balanced");
		expect(parseSensitivity("")).toBe("balanced");
		expect(parseSensitivity("nonsense")).toBe("balanced");
	});
});

describe("confidence classification", () => {
	it("buckets by the 0.66 / 0.4 thresholds", () => {
		expect(confidenceLabel(0.7)).toBe("Strong");
		expect(confidenceLabel(0.5)).toBe("Likely");
		expect(confidenceLabel(0.2)).toBe("Uncertain");
		expect(confidenceTextClass(0.7)).toBe("text-success");
		expect(confidenceTextClass(0.5)).toBe("text-warning");
		expect(confidenceTextClass(0.2)).toBe("text-muted-foreground");
		expect(confidenceBarClass(0.7)).toBe("bg-success");
		expect(confidenceBarClass(0.39)).toBe("bg-muted-foreground/60");
	});
});

describe("cutBounds", () => {
	it("insets by the padding for a long segment", () => {
		// pad = min(0.12, 10/3) = 0.12
		expect(cutBounds(0, 10)).toEqual({ start: 0.12, end: 9.88 });
	});
	it("caps the pad at a third of the segment", () => {
		// segment 0.9s → pad = min(0.12, 0.3) = 0.12 → 0.12..0.78 (len 0.66 ≥ 0.2)
		expect(cutBounds(0, 0.9)).toEqual({ start: 0.12, end: 0.78 });
	});
	it("returns null when the padded region is too short", () => {
		// 0.4s segment → pad min(0.12, 0.133)=0.12 → 0.12..0.28 len 0.16 < 0.2
		expect(cutBounds(0, 0.4)).toBeNull();
	});
});
