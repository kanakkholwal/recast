import { describe, expect, it } from "vitest";
import { formatUserCode, initials, planLabel, roleLabel } from "./cloud-signin.logic";

describe("roleLabel", () => {
	it("title-cases, defaults to Member", () => {
		expect(roleLabel("owner")).toBe("Owner");
		expect(roleLabel("")).toBe("Member");
	});
});

describe("planLabel", () => {
	it("maps known plans, defaults Free", () => {
		expect(planLabel("pro")).toBe("Pro");
		expect(planLabel("enterprise")).toBe("Enterprise");
		expect(planLabel("anything")).toBe("Free");
	});
});

describe("initials", () => {
	it("uses two name parts, then a prefix, then ?", () => {
		expect(initials("Kanak Kholwal", null)).toBe("KK");
		expect(initials("Kanak", null)).toBe("KA");
		expect(initials(null, "kanak@x.com")).toBe("KA");
		expect(initials(null, null)).toBe("?");
	});
});

describe("formatUserCode", () => {
	it("splits into two halves", () => {
		expect(formatUserCode("ABCDEFGH")).toBe("ABCD-EFGH");
		expect(formatUserCode("ab-cd")).toBe("ABCD"); // <=4 after cleaning
	});
});
