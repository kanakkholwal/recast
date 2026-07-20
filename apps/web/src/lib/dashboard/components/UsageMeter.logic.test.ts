import { describe, expect, it } from "vitest";
import { usageTone } from "./UsageMeter.logic";

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

	it("stays neutral at any percentage when uncapped (Enterprise)", () => {
		expect(usageTone(0, false)).toBe("neutral");
		expect(usageTone(95, false)).toBe("neutral");
		expect(usageTone(100, false)).toBe("neutral");
	});
});
