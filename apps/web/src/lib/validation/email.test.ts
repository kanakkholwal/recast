import { describe, expect, it } from "vitest";
import { emailField, isValidEmail, normalizeEmail } from "./email";

describe("normalizeEmail", () => {
	it("produces the canonical stored form", () => {
		// share_member.email and user.email are both written normalized, so
		// allowlist lookups only match if input is normalized the same way.
		expect(normalizeEmail("  Kanak@Example.COM ")).toBe("kanak@example.com");
	});
});

describe("isValidEmail", () => {
	it("accepts real-world addresses", () => {
		for (const a of [
			"kanak@example.com",
			"a+tag@gmail.com",
			"first.last@sub.domain.co.uk",
			"user_name@example-site.io",
			"x@y.dev",
			"kanak@xn--80ak6aa92e.com", // punycode IDN
			"  Kanak@Example.COM  ",
		]) {
			expect(isValidEmail(a), a).toBe(true);
		}
	});

	it("rejects the malformed cases the old hand-rolled regex let through", () => {
		// Each of these passed /^[^\s@]+@[^\s@]+\.[^\s@]+$/ and would have been
		// written to the DB as a permanently unreachable address.
		for (const a of [
			"a@b..com",
			"a@-example.com",
			".a@example.com",
			"a.@example.com",
			"a@example..",
			"a@.example.com",
		]) {
			expect(isValidEmail(a), a).toBe(false);
		}
	});

	it("rejects the obvious junk", () => {
		for (const a of ["", "plainaddress", "a@b", "@example.com", "a b@example.com"]) {
			expect(isValidEmail(a), a).toBe(false);
		}
	});
});

describe("emailField", () => {
	it("normalizes before validating, so callers get the canonical form", () => {
		const r = emailField().safeParse("  Kanak@Example.COM ");
		expect(r.success && r.data).toBe("kanak@example.com");
	});

	it("rejects a non-string instead of coercing it", () => {
		// Unlike the lenient metadata fields, email gates the request.
		expect(emailField().safeParse(42).success).toBe(false);
		expect(emailField().safeParse(undefined).success).toBe(false);
		expect(emailField().safeParse(null).success).toBe(false);
	});

	it("carries the caller's message so endpoint copy stays its own", () => {
		const r = emailField("Invalid email").safeParse("nope");
		expect(r.success).toBe(false);
		if (!r.success) expect(r.error.issues[0]?.message).toBe("Invalid email");
	});
});
