import { describe, expect, it } from "vitest";
import { dbForVolume, envelopePath } from "./audio-panel.logic";

describe("dbForVolume", () => {
	it("is −∞ at or below zero", () => {
		expect(dbForVolume(0)).toBe("−∞ dB");
		expect(dbForVolume(-5)).toBe("−∞ dB");
	});
	it("is 0.0 dB at 100% and signed otherwise", () => {
		expect(dbForVolume(100)).toBe("0.0 dB");
		expect(dbForVolume(200)).toBe("+6.0 dB");
		expect(dbForVolume(50)).toBe("-6.0 dB");
	});
});

describe("envelopePath", () => {
	it("draws a flat top with no fades", () => {
		// fades 0 → xIn 0, xOut 100
		expect(envelopePath(0, 0, 10)).toBe("M 0 22 L 0.00 2 L 100.00 2 L 100 22");
	});
	it("caps each fade at half the clip", () => {
		// clip 10s, fadeIn 8 → capped to 5 → xIn 50
		const p = envelopePath(8, 0, 10);
		expect(p).toContain("L 50.00 2");
	});
});
