import { describe, expect, it } from "vitest";
import {
	canResetPassword,
	canSignUp,
	passwordsMatch,
	scorePasswordStrength,
	STRENGTH_COLORS,
	STRENGTH_LABELS,
} from "./password.logic";

describe("scorePasswordStrength", () => {
	it("scores each rule independently", () => {
		expect(scorePasswordStrength("")).toBe(0);
		expect(scorePasswordStrength("shortie")).toBe(0); // under 8, one case, no digit/symbol
		expect(scorePasswordStrength("password")).toBe(1); // length only
		expect(scorePasswordStrength("Password")).toBe(2); // + mixed case
		expect(scorePasswordStrength("Password1")).toBe(3); // + digit
		expect(scorePasswordStrength("Password1!")).toBe(4); // + symbol
	});

	it("never exceeds the label and colour tables", () => {
		const max = scorePasswordStrength("Password1!");
		expect(max).toBeLessThan(STRENGTH_LABELS.length);
		expect(max).toBeLessThan(STRENGTH_COLORS.length);
	});
});

describe("passwordsMatch", () => {
	it("stays true while the confirm field is untouched", () => {
		// The mismatch hint must stay hidden until the user starts typing.
		expect(passwordsMatch("secret123", "")).toBe(true);
	});

	it("compares once the confirm field has content", () => {
		expect(passwordsMatch("secret123", "secret123")).toBe(true);
		expect(passwordsMatch("secret123", "secret12")).toBe(false);
	});
});

describe("canSignUp", () => {
	const valid = {
		name: "Kanak",
		email: "kanak@example.com",
		password: "secret123",
		confirmPassword: "secret123",
		agreed: true,
	};

	it("accepts a complete, agreed, matching form", () => {
		expect(canSignUp(valid)).toBe(true);
	});

	it("rejects each missing requirement", () => {
		expect(canSignUp({ ...valid, name: "   " })).toBe(false);
		expect(canSignUp({ ...valid, email: "  " })).toBe(false);
		expect(canSignUp({ ...valid, password: "short", confirmPassword: "short" })).toBe(false);
		expect(canSignUp({ ...valid, confirmPassword: "different" })).toBe(false);
		expect(canSignUp({ ...valid, agreed: false })).toBe(false);
	});
});

describe("canResetPassword", () => {
	it("requires a long enough, matching pair", () => {
		expect(canResetPassword({ password: "secret123", confirmPassword: "secret123" })).toBe(true);
		expect(canResetPassword({ password: "short", confirmPassword: "short" })).toBe(false);
		expect(canResetPassword({ password: "secret123", confirmPassword: "secret124" })).toBe(false);
	});
});
