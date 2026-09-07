import { describe, expect, it } from "vitest";
import { monogram, sharesMeter } from "./sidebar-account.logic";

describe("monogram", () => {
	it("takes first + last initials for multi-word names", () => {
		expect(monogram("Kanak Kholwal")).toBe("KK");
		expect(monogram("Acme Design Team")).toBe("AT");
	});

	it("takes the first two letters for a single word", () => {
		expect(monogram("Recast")).toBe("RE");
	});

	it("falls back to a placeholder when empty", () => {
		expect(monogram("   ")).toBe("?");
	});
});

describe("sharesMeter", () => {
	it("returns a clamped percentage against a finite limit", () => {
		expect(sharesMeter(3, 10)).toEqual({ label: "3 of 10 shares", pct: 30 });
		expect(sharesMeter(20, 10)?.pct).toBe(100);
	});

	it("returns null when there is no finite limit", () => {
		expect(sharesMeter(3, null)).toBeNull();
		expect(sharesMeter(3, 0)).toBeNull();
		expect(sharesMeter(3, undefined)).toBeNull();
	});
});
